//! The live coordinator: the only place that ties the read-only pieces to a
//! broadcast. Everything is opt-in behind `ARB_LIVE_SEND=true`.
//!
//! Order is deliberate. Every expensive check, including the exact-sender
//! `eth_call`, runs BEFORE the durable reservation is claimed, so a failed
//! candidate never burns the single shot.
use super::{activation, quote, reserve, sender};
use crate::arbitrage::{
    candidate::ReadOnlyCandidate,
    one_shot::{assess_after_reservation, assess_before_reservation},
    preflight::{prepare_v2_flash_loan, ExecutionLimits, GasQuote as PreflightGas},
    state::ReserveCache,
    Cycle,
};
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
        Self {
            stage,
            detail: detail.into(),
        }
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
    required(name)?
        .trim()
        .parse()
        .with_context(|| format!("{name} is not an address"))
}

fn u64_var(name: &str, default: u64) -> Result<u64> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value
            .trim()
            .parse()
            .with_context(|| format!("{name} must be u64")),
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
        ensure!(!config.tip_wei.is_zero(), "ARB_TIP_WEI must be positive");
        ensure!(
            config.max_flash_loan > U256::ZERO,
            "ARB_MAX_FLASH_LOAN must be positive"
        );
        Ok(Some(config))
    }
}

/// Wei of gas into units of `asset`, through the ETH/USD quote and the asset decimals.
fn wei_to_asset(wei: U256, eth_usd_e8: U256, decimals: u8) -> U256 {
    let numerator = wei
        .saturating_mul(eth_usd_e8)
        .saturating_mul(U256::from(10u64).pow(U256::from(decimals as u64)));
    numerator / U256::from(10u64).pow(U256::from(26u64))
}

/// One candidate, end to end. Returns the stage that stopped it.
pub async fn attempt<P: Provider<Ethereum> + Clone>(
    provider: &P,
    signer: &PrivateKeySigner,
    config: &LiveConfig,
    cache: &ReserveCache,
    cycle: &Cycle,
    block: u64,
    base_fee_per_gas: u64,
) -> Result<Attempt> {
    let max_fee = U256::from(base_fee_per_gas) * U256::from(2u64) + config.tip_wei;
    let gas_spend = U256::from(config.gas_units) * max_fee;

    let header = provider
        .get_block_by_number(block.into())
        .await?
        .context("pinned block unavailable")?;
    let limits = ExecutionLimits {
        current_block: block,
        max_pool_age_blocks: 0,
        max_hops: 3,
        max_flash_loan: config.max_flash_loan,
        max_gas_spend_wei: gas_spend,
        premium_bps: config.premium_bps,
        min_profit_after_gas_in_asset: config.min_profit_after_gas_in_asset,
    };
    // First pass only learns which asset the loan uses: the gas figure is
    // still wei here, which is correct only for an 18-decimal asset.
    let probe = PreflightGas {
        gas_spend_wei: gas_spend,
        gas_cost_in_asset: gas_spend,
        quoted_at_block: block,
    };
    let borrowed = match prepare_v2_flash_loan(cache, cycle, &limits, Some(&probe)) {
        Ok(plan) => plan.asset,
        Err(error) => return Ok(Attempt::stop(Stage::Prepare, error.to_string())),
    };

    let gas_quote = match quote::read_eth_usd(provider, block, header.header.timestamp).await {
        Ok(gas_quote) => gas_quote,
        Err(error) => return Ok(Attempt::stop(Stage::Quote, error.to_string())),
    };
    let quoted_at = std::time::Instant::now();

    let decimals = crate::arbitrage::rpc::load_token_decimals(provider, [borrowed])
        .await
        .get(&borrowed)
        .copied()
        .unwrap_or(18);
    let gas_cost_in_asset = wei_to_asset(gas_spend, gas_quote.eth_usd_e8, decimals);
    let cost = PreflightGas {
        gas_spend_wei: gas_spend,
        gas_cost_in_asset,
        quoted_at_block: block,
    };
    let prepared = match prepare_v2_flash_loan(cache, cycle, &limits, Some(&cost)) {
        Ok(plan) => plan,
        Err(error) => return Ok(Attempt::stop(Stage::Prepare, error.to_string())),
    };
    let net = prepared
        .expected_gross_profit
        .saturating_sub(gas_cost_in_asset);
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
    let check = match quote::build_check(
        1,
        config.gas_units,
        max_fee,
        &gas_quote.aged(quoted_at.elapsed().as_secs()),
        evidence,
    ) {
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
