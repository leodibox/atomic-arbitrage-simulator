//! Strict, side-effect-free preflight for the existing Aave/V2 flash-loan
//! contract. This prepares calldata but NEVER signs, submits, or spends ETH.
//!
//! A scanner cycle may contain V3 hops; the deployed Solidity executor only
//! implements V2-style pair.swap() and assumes the canonical 30 bps fee.
//! An on-chain caller must still check contract bytecode/owner, signed chain ID,
//! fresh on-chain quotes, eth_call/estimateGas, nonce, and private submission
//! before using this plan. This module is NOT a live-trading approval.

use super::cycles::Cycle;
use super::state::ReserveCache;
use alloy::primitives::{Address, Bytes, U256};
use alloy::sol;
use alloy::sol_types::SolCall;
use anyhow::{bail, ensure, Result};
use std::collections::HashSet;

sol! {
    function executeArbitrage(
        address asset,
        uint256 amount,
        bytes path,
        uint256 minProfit
    ) external;
}

#[derive(Debug, Clone)]
pub struct GasQuote {
    /// Estimated gas paid in ETH, prior to token-denominated conversion.
    pub gas_spend_wei: U256,
    /// Conservative cost of that ETH in the flash-loan *start asset*.
    pub gas_cost_in_asset: U256,
    /// Ethereum block used to independently price both estimates.
    pub quoted_at_block: u64,
}

#[derive(Debug, Clone)]
pub struct ExecutionLimits {
    pub current_block: u64,
    pub max_pool_age_blocks: u64,
    pub max_hops: usize,
    pub max_flash_loan: U256,
    pub max_gas_spend_wei: U256,
    pub premium_bps: u64,
    pub min_profit_after_gas_in_asset: U256,
}

#[derive(Debug, Clone)]
pub struct PreparedV2FlashLoan {
    pub asset: Address,
    pub amount: U256,
    pub pair_path: Vec<u8>,
    pub min_profit_onchain: U256,
    pub calldata: Vec<u8>,
    pub quoted_at_block: u64,
    pub expected_gross_profit: U256,
}

/// Validate local execution compatibility and build calldata for
/// FlashLoanArb.executeArbitrage(). Passing here is not enough to send:
/// the caller MUST independently simulate the exact transaction at the latest
/// block and verify gas in ETH, Aave premium, wallet limits, contract bytecode,
/// chain ID, deadline, and profit before signing.
pub fn prepare_v2_flash_loan(
    cache: &ReserveCache,
    cycle: &Cycle,
    limits: &ExecutionLimits,
    cost: Option<&GasQuote>,
) -> Result<PreparedV2FlashLoan> {
    let cost = cost.ok_or_else(|| anyhow::anyhow!("gas cost quote missing; fail closed"))?;
    ensure!(limits.current_block > 0, "no current block");
    ensure!((2..=3).contains(&limits.max_hops), "unsupported hop limit");
    ensure!(cycle.hops.len() >= 2 && cycle.hops.len() <= limits.max_hops, "invalid route length");
    ensure!(cycle.start != Address::ZERO, "zero loan asset");
    ensure!(!cycle.amount_in.is_zero() && cycle.amount_in <= limits.max_flash_loan, "loan amount exceeds cap");
    ensure!(!limits.min_profit_after_gas_in_asset.is_zero(), "minimum net profit must be positive");
    ensure!((1..=1_000).contains(&limits.premium_bps), "invalid flash-loan premium");
    ensure!(cost.quoted_at_block == limits.current_block, "gas quote is not for the current block");
    ensure!(!cost.gas_cost_in_asset.is_zero(), "zero/unknown gas conversion is forbidden");
    ensure!(!cost.gas_spend_wei.is_zero() && cost.gas_spend_wei <= limits.max_gas_spend_wei,
        "gas spend exceeds permitted ETH budget");

    let mut path = Vec::with_capacity(21 * cycle.hops.len());
    let mut seen_pairs = HashSet::new();
    let mut current_token = cycle.start;
    let mut current_amount = cycle.amount_in;
    for hop in &cycle.hops {
        ensure!(seen_pairs.insert(hop.pair), "duplicate pair in route");
        // Crucial: get() excludes V3 pools, whose swap() has different ABI.
        let Some(pool) = cache.get(&hop.pair) else {
            bail!("unknown or V3 pool in a V2-only executable route");
        };
        ensure!(pool.fee_bps == 30, "V2 executor hardcodes 30 bps swap fees");
        ensure!(pool.block > 0 && pool.block <= limits.current_block
            && limits.current_block - pool.block <= limits.max_pool_age_blocks,
            "pool state is stale or from the future");
        ensure!(hop.token_in == current_token && hop.amount_in == current_amount,
            "discontinuous route");
        let zero_for_one = if current_token == pool.token0 {
            true
        } else if current_token == pool.token1 {
            false
        } else {
            bail!("route input does not match pair token0/token1");
        };
        let (expected_token, expected_amount) = pool.quote(current_token, current_amount)
            .ok_or_else(|| anyhow::anyhow!("pair quote unavailable"))?;
        ensure!(hop.token_out == expected_token && hop.amount_out == expected_amount,
            "route output does not match current pool quote");
        path.extend_from_slice(hop.pair.as_slice());
        path.push(u8::from(zero_for_one));
        current_token = expected_token;
        current_amount = expected_amount;
    }
    ensure!(current_token == cycle.start, "route does not return to loan asset");
    ensure!(current_amount == cycle.amount_out, "outdated final amount");
    let profit = current_amount.checked_sub(cycle.amount_in)
        .ok_or_else(|| anyhow::anyhow!("route has no gross profit"))?;
    ensure!(profit == cycle.gross_profit, "inconsistent gross profit");

    // Round the Aave fee UP. Actual premium must still be checked on-chain.
    let premium = (cycle.amount_in * U256::from(limits.premium_bps) + U256::from(9_999u64))
        / U256::from(10_000u64);
    // minProfit on the contract is paid in the start token. The wallet pays gas
    // in ETH, therefore insist on enough asset surplus to cover priced gas.
    let min_profit_onchain = cost.gas_cost_in_asset + limits.min_profit_after_gas_in_asset;
    ensure!(profit > premium + min_profit_onchain,
        "gross profit cannot cover Aave premium, gas estimate and net floor");

    let calldata = executeArbitrageCall {
        asset: cycle.start,
        amount: cycle.amount_in,
        path: Bytes::from(path.clone()),
        minProfit: min_profit_onchain,
    }.abi_encode();

    Ok(PreparedV2FlashLoan {
        asset: cycle.start,
        amount: cycle.amount_in,
        pair_path: path,
        min_profit_onchain,
        calldata,
        quoted_at_block: limits.current_block,
        expected_gross_profit: profit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arbitrage::{find_cycles, PoolState};
    use alloy::primitives::address;

    fn fixture() -> (ReserveCache, Cycle, ExecutionLimits, GasQuote) {
        let usdc = address!("0000000000000000000000000000000000000001");
        let weth = address!("0000000000000000000000000000000000000002");
        let mut cache = ReserveCache::default();
        cache.insert(PoolState {
            pair: address!("0000000000000000000000000000000000000010"),
            token0: usdc, token1: weth,
            reserve0: U256::from(1_000_000u64),
            reserve1: U256::from(3_000_000u64),
            fee_bps: 30, block: 100,
        });
        cache.insert(PoolState {
            pair: address!("0000000000000000000000000000000000000020"),
            token0: weth, token1: usdc,
            reserve0: U256::from(1_000_000u64),
            reserve1: U256::from(900_000u64),
            fee_bps: 30, block: 100,
        });
        let cycle = find_cycles(&cache, &[usdc], U256::from(1_000u64), 3, U256::ZERO)
            .into_iter().find(|c| c.gross_profit > U256::from(100u64)).unwrap();
        let limits = ExecutionLimits {
            current_block: 100,
            max_pool_age_blocks: 1,
            max_hops: 3,
            max_flash_loan: U256::from(10_000u64),
            max_gas_spend_wei: U256::from(10_000u64),
            premium_bps: 5,
            min_profit_after_gas_in_asset: U256::from(50u64),
        };
        let gas = GasQuote {
            gas_spend_wei: U256::from(100u64),
            gas_cost_in_asset: U256::from(20u64),
            quoted_at_block: 100,
        };
        (cache, cycle, limits, gas)
    }

    #[test]
    fn builds_v2_only_contract_calldata() {
        let (cache, cycle, limits, gas) = fixture();
        let plan = prepare_v2_flash_loan(&cache, &cycle, &limits, Some(&gas)).unwrap();
        assert_eq!(plan.pair_path.len(), 42);
        assert_eq!(plan.pair_path[20], 1);
        assert_eq!(plan.pair_path[41], 1); // second pair is token0=weth, takes weth
        assert_eq!(plan.min_profit_onchain, U256::from(70u64));
        assert_eq!(&plan.calldata[..4],
            &alloy::primitives::keccak256("executeArbitrage(address,uint256,bytes,uint256)").as_slice()[..4]);
    }

    #[test]
    fn refuses_unpriced_gas() {
        let (cache, cycle, limits, gas) = fixture();
        assert!(prepare_v2_flash_loan(&cache, &cycle, &limits, None).is_err());
        let mut wrong = gas.clone();
        wrong.gas_cost_in_asset = U256::ZERO;
        assert!(prepare_v2_flash_loan(&cache, &cycle, &limits, Some(&wrong)).is_err());
        wrong.gas_cost_in_asset = U256::from(20u64);
        wrong.quoted_at_block = 99;
        assert!(prepare_v2_flash_loan(&cache, &cycle, &limits, Some(&wrong)).is_err());
    }

    #[test]
    fn refuses_stale_state_and_oversized_loan() {
        let (cache, cycle, mut limits, gas) = fixture();
        limits.current_block = 103;
        assert!(prepare_v2_flash_loan(&cache, &cycle, &limits, Some(&gas)).is_err());
        limits.current_block = 100;
        limits.max_flash_loan = U256::from(500u64);
        assert!(prepare_v2_flash_loan(&cache, &cycle, &limits, Some(&gas)).is_err());
    }

    #[test]
    fn refuses_noncanonical_v2_fee_and_modified_quote() {
        let (mut cache, mut cycle, limits, gas) = fixture();
        cache.get_mut(&cycle.hops[0].pair).unwrap().fee_bps = 25;
        assert!(prepare_v2_flash_loan(&cache, &cycle, &limits, Some(&gas)).is_err());
        cache.get_mut(&cycle.hops[0].pair).unwrap().fee_bps = 30;
        cycle.hops[0].amount_out += U256::from(1u64);
        assert!(prepare_v2_flash_loan(&cache, &cycle, &limits, Some(&gas)).is_err());
    }

    #[test]
    fn refuses_v3_or_missing_pair_in_executable_path() {
        let (cache, mut cycle, limits, gas) = fixture();
        cycle.hops[0].pair = address!("0000000000000000000000000000000000000030");
        assert!(prepare_v2_flash_loan(&cache, &cycle, &limits, Some(&gas)).is_err());
    }

    #[test]
    fn refuses_negative_profit_or_excessive_gas_budget() {
        let (cache, mut cycle, mut limits, mut gas) = fixture();
        gas.gas_spend_wei = U256::from(20_000u64);
        assert!(prepare_v2_flash_loan(&cache, &cycle, &limits, Some(&gas)).is_err());
        gas.gas_spend_wei = U256::from(100u64);
        limits.min_profit_after_gas_in_asset = U256::from(5_000u64);
        assert!(prepare_v2_flash_loan(&cache, &cycle, &limits, Some(&gas)).is_err());
        limits.min_profit_after_gas_in_asset = U256::from(50u64);
        cycle.gross_profit = U256::ZERO;
        assert!(prepare_v2_flash_loan(&cache, &cycle, &limits, Some(&gas)).is_err());
    }
}
