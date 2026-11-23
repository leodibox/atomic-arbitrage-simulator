//! Periodic READ-ONLY ETH/USD budget diagnostic for Ethereum mainnet.
//! This is not execution approval; never sign, send, reserve or deploy here.
use crate::arbitrage::one_shot::{maximum_gas_cost_usd_e8, MAX_TOTAL_GAS_USD_E8};
use alloy::{
    network::Ethereum,
    primitives::{address, U256},
    providers::Provider,
    sol,
};
use anyhow::{ensure, Context, Result};

sol! {
    #[sol(rpc)]
    interface ChainlinkEthUsd {
        function decimals() external view returns (uint8);
        function latestRoundData() external view returns (
            uint80 roundId, int256 answer, uint256 startedAt,
            uint256 updatedAt, uint80 answeredInRound
        );
    }
}

const ORACLE_MAX_AGE_SECS_FOR_DIAGNOSTIC: u64 = 3600;

pub async fn read_gas_usd_diagnostic<P: Provider<Ethereum> + Clone>(
    provider: &P,
    block: u64,
    gas_limit: u64,
    max_fee_wei: U256,
    chain_timestamp: u64,
) -> Result<(U256, u64, bool)> {
    ensure!(
        provider.get_chain_id().await? == 1,
        "ETH/USD oracle diagnostic only supports Ethereum mainnet"
    );
    let aggregator = ChainlinkEthUsd::new(
        address!("5f4eC3Df9cbd43714FE2740f5E3616155c5b8419"),
        provider.clone(),
    );
    let decimals = aggregator.decimals().block(block.into()).call().await?;
    ensure!(decimals == 8, "unexpected Chainlink ETH/USD decimals");
    let round = aggregator
        .latestRoundData()
        .block(block.into())
        .call()
        .await?;
    let answer = round
        .answer
        .to_string()
        .parse::<U256>()
        .context("nonpositive or invalid ETH/USD price")?;
    ensure!(!answer.is_zero(), "ETH/USD price is zero");
    let updated = u64::try_from(round.updatedAt).context("ETH/USD timestamp out of range")?;
    ensure!(
        !round.roundId.is_zero() && round.answeredInRound >= round.roundId,
        "invalid ETH/USD Chainlink round"
    );
    ensure!(
        updated != 0 && updated <= chain_timestamp,
        "ETH/USD timestamp invalid"
    );
    let age = chain_timestamp - updated;
    ensure!(
        age <= ORACLE_MAX_AGE_SECS_FOR_DIAGNOSTIC,
        "ETH/USD oracle stale; not an execution quote"
    );
    let quote_usd_e8 = maximum_gas_cost_usd_e8(gas_limit, max_fee_wei, answer)?;
    Ok((
        quote_usd_e8,
        age,
        quote_usd_e8 <= U256::from(MAX_TOTAL_GAS_USD_E8),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_budget_reuses_exact_one_shot_worst_case_math() {
        let cap = maximum_gas_cost_usd_e8(
            100_000,
            U256::from(10_000_000_000u64),
            U256::from(300_000_000_000u64),
        )
        .unwrap();
        assert_eq!(cap, U256::from(330_000_000u64));
        assert!(cap <= U256::from(MAX_TOTAL_GAS_USD_E8));
    }
    #[test]
    fn realistic_expensive_gas_exceeds_budget() {
        let cap = maximum_gas_cost_usd_e8(
            1_000_000,
            U256::from(12_000_000_000u64),
            U256::from(300_000_000_000u64),
        )
        .unwrap();
        assert!(cap > U256::from(MAX_TOTAL_GAS_USD_E8));
    }
}
