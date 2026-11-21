//! Fresh, chain-scoped ETH/USD quote for the $5 one-shot gas gate.
//! READ-ONLY: never signs, sends or reserves.
//!
//! Freshness is split on purpose:
//! * `fetch_age_secs` is the age of THIS read. It must stay <= 30s, because a
//!   cached answer is not a fresh quote.
//! * `round_age_secs` is the Chainlink round age on chain. Mainnet ETH/USD has
//!   a 1h heartbeat and a 0.5% deviation threshold, so demanding 30s there
//!   would reject nearly every read.
//! The 10% FX margin inside `one_shot::maximum_gas_cost_usd_e8` absorbs the
//! residual staleness of the round itself.
use crate::arbitrage::one_shot::OneShotCheck;
use alloy::{
    network::Ethereum,
    primitives::{address, Address, U256},
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

pub const CHAINLINK_ETH_USD: Address =
    address!("5f4eC3Df9cbd43714FE2740f5E3616155c5b8419");
pub const FETCH_MAX_AGE_SECS: u64 = 30;
pub const ROUND_MAX_AGE_SECS: u64 = 3600;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GasQuote {
    pub eth_usd_e8: U256,
    pub fetch_age_secs: u64,
    pub round_age_secs: u64,
}

impl GasQuote {
    /// Simulate elapsed time between reading the quote and using it.
    pub fn aged(&self, elapsed_secs: u64) -> Self {
        let mut next = self.clone();
        next.fetch_age_secs = next.fetch_age_secs.saturating_add(elapsed_secs);
        next
    }

    pub fn is_fresh(&self) -> bool {
        self.fetch_age_secs <= FETCH_MAX_AGE_SECS
    }
}

/// Read the canonical Chainlink ETH/USD aggregator at a pinned block.
/// `chain_timestamp` must be the pinned block timestamp, never wall clock.
pub async fn read_eth_usd<P: Provider<Ethereum> + Clone>(
    provider: &P,
    block: u64,
    chain_timestamp: u64,
) -> Result<GasQuote> {
    ensure!(
        provider.get_chain_id().await? == 1,
        "ETH/USD quote is Ethereum mainnet only"
    );
    ensure!(block > 0, "quote requires a pinned block");
    ensure!(chain_timestamp > 0, "quote requires the pinned block timestamp");
    let aggregator = ChainlinkEthUsd::new(CHAINLINK_ETH_USD, provider.clone());
    let decimals = aggregator.decimals().block(block.into()).call().await?;
    ensure!(decimals == 8, "unexpected Chainlink ETH/USD decimals");
    let round = aggregator.latestRoundData().block(block.into()).call().await?;
    let answer = round
        .answer
        .to_string()
        .parse::<U256>()
        .context("nonpositive or invalid ETH/USD answer")?;
    ensure!(!answer.is_zero(), "ETH/USD answer is zero");
    ensure!(
        !round.roundId.is_zero() && round.answeredInRound >= round.roundId,
        "invalid Chainlink ETH/USD round"
    );
    let updated = u64::try_from(round.updatedAt).context("ETH/USD timestamp out of range")?;
    ensure!(
        updated != 0 && updated <= chain_timestamp,
        "ETH/USD timestamp invalid"
    );
    let round_age_secs = chain_timestamp - updated;
    ensure!(
        round_age_secs <= ROUND_MAX_AGE_SECS,
        "ETH/USD oracle round is stale"
    );
    Ok(GasQuote {
        eth_usd_e8: answer,
        fetch_age_secs: 0,
        round_age_secs,
    })
}

/// Evidence the coordinator must supply. Booleans are not proof on their own;
/// the coordinator owns how each one is established.
#[derive(Clone, Copy, Debug, Default)]
pub struct Evidence {
    pub exact_sender_simulation_succeeded: bool,
    pub nonce_and_contract_verified: bool,
    pub onchain_pair_and_loan_caps_verified: bool,
    pub route_profitable_after_all_costs: bool,
}

/// Assemble the gate input. Rejects an aged quote before it can reach
/// `one_shot::assess_before_reservation`.
pub fn build_check(
    chain_id: u64,
    gas_limit: u64,
    max_fee_per_gas_wei: U256,
    quote: &GasQuote,
    evidence: Evidence,
) -> Result<OneShotCheck> {
    ensure!(
        quote.is_fresh(),
        "ETH/USD quote older than {FETCH_MAX_AGE_SECS}s"
    );
    ensure!(
        quote.round_age_secs <= ROUND_MAX_AGE_SECS,
        "ETH/USD oracle round is stale"
    );
    Ok(OneShotCheck {
        chain_id,
        gas_limit,
        max_fee_per_gas_wei,
        eth_usd_e8: quote.eth_usd_e8,
        price_age_seconds: quote.fetch_age_secs,
        exact_sender_simulation_succeeded: evidence.exact_sender_simulation_succeeded,
        nonce_and_contract_verified: evidence.nonce_and_contract_verified,
        onchain_pair_and_loan_caps_verified: evidence.onchain_pair_and_loan_caps_verified,
        route_profitable_after_all_costs: evidence.route_profitable_after_all_costs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arbitrage::one_shot::assess_before_reservation;

    fn quote() -> GasQuote {
        GasQuote {
            eth_usd_e8: U256::from(300_000_000_000u64),
            fetch_age_secs: 5,
            round_age_secs: 600,
        }
    }

    fn evidence() -> Evidence {
        Evidence {
            exact_sender_simulation_succeeded: true,
            nonce_and_contract_verified: true,
            onchain_pair_and_loan_caps_verified: true,
            route_profitable_after_all_costs: true,
        }
    }

    #[test]
    fn fresh_quote_with_complete_evidence_reaches_the_gate() {
        let check = build_check(
            1, 100_000, U256::from(10_000_000_000u64), &quote(), evidence(),
        ).unwrap();
        assert_eq!(
            assess_before_reservation(&check).unwrap(),
            U256::from(330_000_000u64)
        );
    }

    #[test]
    fn stale_fetch_is_rejected_even_with_a_valid_round() {
        let stale = quote().aged(FETCH_MAX_AGE_SECS + 1);
        assert!(build_check(1, 100_000, U256::from(1u64), &stale, evidence()).is_err());
    }

    #[test]
    fn ancient_round_is_rejected() {
        let mut old = quote();
        old.round_age_secs = ROUND_MAX_AGE_SECS + 1;
        assert!(build_check(1, 100_000, U256::from(1u64), &old, evidence()).is_err());
    }

    #[test]
    fn aging_saturates_and_is_additive() {
        let aged = quote().aged(10).aged(u64::MAX);
        assert_eq!(aged.fetch_age_secs, u64::MAX);
        assert_eq!(aged.round_age_secs, 600);
        assert!(!aged.is_fresh());
    }

    #[test]
    fn missing_evidence_still_reaches_the_gate_and_fails_closed() {
        let check = build_check(
            1, 100_000, U256::from(10_000_000_000u64), &quote(), Evidence::default(),
        ).unwrap();
        assert!(assess_before_reservation(&check).is_err());
    }

    #[test]
    fn zero_price_is_rejected_by_the_gate_math() {
        let mut zero = quote();
        zero.eth_usd_e8 = U256::ZERO;
        let check = build_check(
            1, 100_000, U256::from(10_000_000_000u64), &zero, evidence(),
        ).unwrap();
        assert!(assess_before_reservation(&check).is_err());
    }
}
