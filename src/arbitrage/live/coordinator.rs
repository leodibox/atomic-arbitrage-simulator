//! The live coordinator: the only place that ties the read-only pieces to a
//! broadcast. Everything is opt-in behind `ARB_LIVE_SEND=true`.
//!
//! Order is deliberate. Every expensive check, including the exact-sender
//! `eth_call`, runs BEFORE the durable reservation is claimed, so a failed
//! candidate never burns the single shot.
use super::{activation, quote, reserve, sender};
use crate::arbitrage::{
    candidate::ReadOnlyCandidate,
    net::{conservative_net, slippage_haircut, NetScreen},
    one_shot::{assess_after_reservation, assess_before_reservation},
    preflight::{prepare_v2_flash_loan, ExecutionLimits, GasQuote as PreflightGas},
    state::ReserveCache,
    Cycle,
};
use crate::common::constants::WETH;
use alloy::{
    network::Ethereum,
    primitives::{address, Address, B256, U256},
    providers::Provider,
    signers::local::PrivateKeySigner,
};
use anyhow::{bail, ensure, Context, Result};
use std::path::PathBuf;
use tracing::info;

const AAVE_V3_POOL: Address = address!("87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Prepare,
    Quote,
    Activation,
    GasFloor,
    Simulation,
    Gate,
    Reservation,
    Broadcast,
}

#[derive(Debug, Clone)]
pub struct Attempt {
    pub stage: Stage,
    pub detail: String,
}

impl Attempt {
    fn stop(stage: Stage, detail: impl Into<String>) -> Self {
        Self { stage, detail: detail.into() }
    }
}

#[derive(Clone, Debug)]
pub struct LiveConfig {
    pub reserve_dir: PathBuf,
    pub executor: Address,
    pub owner: Address,
    pub recipient: Address,
    pub runtime_keccak: B256,
    pub gas_units: u64,
    pub min_gas_wei: U256,
    pub tip_wei: U256,
    pub min_profit_after_gas_in_asset: U256,
    pub max_flash_loan: U256,
    pub premium_bps: u64,
    pub slippage_bps: u64,
}

fn required(name: &str) -> Result<String> {
    std::env::var(name).with_context(|| format!("missing {name}"))
}

fn address_var(name: &str) -> Result<Address> {
    required(name)?.trim().parse().with_context(|| format!("{name} is not an address"))
}

fn u64_var(name: &str, default: u64) -> Result<u64> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => {
            value.trim().parse().with_context(|| format!("{name} must be u64"))
        }
        _ => Ok(default),
    }
}

fn u256_var(name: &str, default: &str) -> Result<U256> {
    let raw = std::env::var(name).unwrap_or_else(|_| default.to_string());
    U256::from_str_radix(raw.trim(), 10).with_context(|| format!("{name} must be wei"))
}

impl LiveConfig {
    /// `None` means the coordinator is inert and nothing is signed.
    pub fn from_env() -> Result<Option<Self>> {
        let enabled = std::env::var("ARB_LIVE_SEND")
            .map(|value| value.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if !enabled {
            return Ok(None);
        }
        let config = Self {
            reserve_dir: required("ARB_RESERVE_DIR")?.into(),
            executor: address_var("GUARDED_EXECUTOR_ADDRESS")?,
            owner: address_var("EXPECTED_EXECUTOR_OWNER")?,
            recipient: address_var("EXPECTED_PROFIT_RECIPIENT")?,
            runtime_keccak: required("GUARDED_RUNTIME_KECCAK256")?
                .trim()
                .parse()
                .context("GUARDED_RUNTIME_KECCAK256 is not a hash")?,
            gas_units: u64_var("ARB_GAS_UNITS", 900_000)?,
            min_gas_wei: u256_var("ARB_MIN_GAS_WEI", "2000000000000000")?,
            tip_wei: u256_var("ARB_TIP_WEI", "1000000000")?,
            min_profit_after_gas_in_asset: u256_var("ARB_MIN_PROFIT_AFTER_GAS", "1")?,
            max_flash_loan: u256_var("ARB_MAX_FLASH_LOAN", "250000000000000000")?,
            premium_bps: u64_var("ARB_PREMIUM_BPS", 5)?,
            slippage_bps: u64_var("ARB_SLIPPAGE_BPS", 10)?,
        };
        ensure!(config.gas_units > 0, "ARB_GAS_UNITS must be positive");
        ensure!(config.slippage_bps <= 10_000, "ARB_SLIPPAGE_BPS must not exceed 10000");
        ensure!(!config.tip_wei.is_zero(), "ARB_TIP_WEI must be positive");
        ensure!(config.max_flash_loan > U256::ZERO, "ARB_MAX_FLASH_LOAN must be positive");
        Ok(Some(config))
    }
}

fn weth_gas_quote(cycle: &Cycle, gas_spend_wei: U256, block: u64) -> Result<PreflightGas> {
    ensure!(cycle.start == WETH, "live arbitrage requires a WETH loan asset");
    Ok(PreflightGas {
        gas_spend_wei,
        gas_cost_in_asset: gas_spend_wei,
        quoted_at_block: block,
    })
}

/// One candidate, end to end. Returns the stage that stopped it.
pub async fn attempt<P: Provider<Ethereum> + Clone>(
    provider: &P,
    signer: &PrivateKeySigner,
    config: &LiveConfig,
    cache: &ReserveCache,
    cycle: &Cycle,
    screen: &NetScreen,
) -> Result<Attempt> {
    let block = screen.block;
    let max_fee = U256::from(screen.base_fee_per_gas) * U256::from(2u64) + config.tip_wei;
    let gas_spend = U256::from(config.gas_units) * max_fee;
    let cost = weth_gas_quote(cycle, gas_spend, block)?;
    let slippage_bps = config.slippage_bps.max(screen.slippage_bps);
    let slippage = slippage_haircut(cycle.amount_out, slippage_bps)
        .context("invalid slippage haircut")?;
    let net_floor = config.min_profit_after_gas_in_asset.max(screen.min_profit_after_gas);
    // Preflight checks the unbuffered route output, so reserve the haircut in
    // the floor that preflight checks and encodes into the executor's calldata.

    let limits = ExecutionLimits {
        current_block: block,
        max_pool_age_blocks: 0,
        max_hops: 3,
        max_flash_loan: config.max_flash_loan,
        max_gas_spend_wei: gas_spend,
        premium_bps: config.premium_bps.max(screen.premium_bps),
        min_profit_after_gas_in_asset: net_floor.checked_add(slippage)
            .context("slippage-adjusted profit floor overflow")?,
    };
    let prepared = match prepare_v2_flash_loan(cache, cycle, &limits, Some(&cost)) {
        Ok(plan) => plan,
        Err(error) => return Ok(Attempt::stop(Stage::Prepare, error.to_string())),
    };
    let net = conservative_net(
        cycle.amount_out, cycle.amount_in, limits.premium_bps, gas_spend, slippage_bps,
    ).context("live cycle has no modeled net profit")?;
    // The USD quote is used only for the independent cumulative gas budget.
    let gas_quote = match quote::read_eth_usd(provider, block, screen.block_timestamp).await {
        Ok(gas_quote) => gas_quote,
        Err(error) => return Ok(Attempt::stop(Stage::Quote, error.to_string())),
    };
    let quoted_at = std::time::Instant::now();
    let candidate = match ReadOnlyCandidate::from_preflight(1, block, &prepared, net) {
        Ok(candidate) => candidate,
        Err(error) => return Ok(Attempt::stop(Stage::Prepare, error.to_string())),
    };

    let request = activation::ActivationRequest {
        executor: config.executor,
        signer: signer.address(),
        asset: prepared.asset,
        amount: prepared.amount,
        pairs: cycle.hops.iter().map(|hop| hop.pair).collect(),
        expected_owner: config.owner,
        expected_recipient: config.recipient,
        expected_pool: AAVE_V3_POOL,
        expected_runtime_keccak: config.runtime_keccak,
    };
    if let Err(error) = activation::verify_permission(provider, &request).await {
        return Ok(Attempt::stop(Stage::Activation, error.to_string()));
    }

    let balance = match provider.get_balance(signer.address()).await {
        Ok(balance) => balance,
        Err(error) => {
            return Ok(Attempt::stop(
                Stage::GasFloor,
                format!("cannot verify signer ETH balance: {error}"),
            ));
        }
    };
    if balance < config.min_gas_wei {
        return Ok(Attempt::stop(
            Stage::GasFloor,
            format!(
                "signer ETH balance {balance} wei is below ARB_MIN_GAS_WEI ({} wei)",
                config.min_gas_wei
            ),
        ));
    }

    let nonce = provider.get_transaction_count(signer.address()).await?;
    let send_request = sender::SendRequest {
        executor: config.executor,
        calldata: prepared.calldata.clone(),
        gas_limit: config.gas_units,
        max_fee_per_gas_wei: max_fee,
        max_priority_fee_per_gas_wei: config.tip_wei,
        nonce,
        block,
    };
    if let Err(error) = sender::simulate(provider, signer.address(), &send_request).await {
        return Ok(Attempt::stop(Stage::Simulation, error.to_string()));
    }

    let evidence = quote::Evidence {
        exact_sender_simulation_succeeded: true,
        nonce_and_contract_verified: true,
        onchain_pair_and_loan_caps_verified: true,
        route_profitable_after_all_costs: !net.is_zero(),
    };
    let check = match quote::build_check(1, config.gas_units, max_fee, &gas_quote.aged(quoted_at.elapsed().as_secs()), evidence) {
        Ok(check) => check,
        Err(error) => return Ok(Attempt::stop(Stage::Gate, error.to_string())),
    };
    let worst_case = match assess_before_reservation(&check) {
        Ok(worst_case) => worst_case,
        Err(error) => return Ok(Attempt::stop(Stage::Gate, error.to_string())),
    };

    let key = reserve::ReservationKey {
        chain_id: 1,
        sender: signer.address(),
        nonce,
        calldata_hash: candidate.calldata_hash,
        max_fee_per_gas_wei: max_fee,
        gas_limit: config.gas_units,
    };
    let reservation = match reserve::acquire(&config.reserve_dir, &key) {
        Ok(reservation) => reservation,
        Err(error) => return Ok(Attempt::stop(Stage::Reservation, error.to_string())),
    };
    let persisted = reserve::count_attempts(&config.reserve_dir)?;
    if let Err(error) = assess_after_reservation(&check, reservation.is_exact(), persisted) {
        bail!("reservation committed but gate refused: {error}");
    }

    info!(
        nonce,
        calldata_hash = %candidate.calldata_hash,
        worst_case_usd_e8 = %worst_case,
        loan_wei = %prepared.amount,
        "gate passed, broadcasting"
    );
    let hash = sender::sign_and_send(provider, signer, &send_request).await?;
    let outcome = sender::await_receipt(provider, hash, 30).await?;
    Ok(Attempt::stop(
        Stage::Broadcast,
        format!("{hash:#x} {outcome:?}"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weth_gas_cost_uses_the_same_wei_as_native_eth() {
        let mut cycle = Cycle {
            start: WETH,
            amount_in: U256::ZERO,
            amount_out: U256::ZERO,
            gross_profit: U256::ZERO,
            hops: Vec::new(),
            freshest_block: 100,
        };
        let gas = U256::from(2_000_000_000_000_000u64);
        let quote = weth_gas_quote(&cycle, gas, 100).unwrap();
        assert_eq!(quote.gas_spend_wei, gas);
        assert_eq!(quote.gas_cost_in_asset, gas);

        cycle.start = Address::repeat_byte(1);
        assert!(weth_gas_quote(&cycle, gas, 100).is_err());
    }
}
