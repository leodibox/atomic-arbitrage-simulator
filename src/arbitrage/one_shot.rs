//! Deterministic, fail-closed approval gate for a *future* automatic sender.
//! This module does NOT sign, submit, deploy or manage a wallet.
//!
//! A production caller MUST obtain a globally exclusive, DURABLE single-use
//! attempt reservation before invoking this gate. An in-memory bool cannot
//! guarantee one attempt across worker restarts or concurrent instances.
//! The USD quote must be independently verified, fresh and chain-scoped.
//! The transaction's gasLimit and maxFeePerGas MUST equal the gated values.

use alloy::primitives::U256;
use anyhow::{ensure, Context, Result};

/// $5 in USD with 8 decimal places.
pub const MAX_TOTAL_GAS_USD_E8: u64 = 500_000_000;
const ETH_WEI: u64 = 1_000_000_000_000_000_000;
const MAX_PRICE_AGE_SECONDS: u64 = 30;

/// Values established by the execution coordinator, NOT user input or logs.
/// A price of $3,000 ETH is represented as 300_000_000_000.
#[derive(Debug, Clone)]
pub struct OneShotCheck {
    pub chain_id: u64,
    pub gas_limit: u64,
    pub max_fee_per_gas_wei: U256,
    pub eth_usd_e8: U256,
    pub price_age_seconds: u64,
    pub exact_sender_simulation_succeeded: bool,
    pub nonce_and_contract_verified: bool,
    pub onchain_pair_and_loan_caps_verified: bool,
    pub route_profitable_after_all_costs: bool,
}

/// Worst-case gas in USD-e8, rounded UP with a 10% FX safety margin.
/// Uses maxFeePerGas (not the current baseFee or an average gas estimate).
pub fn maximum_gas_cost_usd_e8(
    gas_limit: u64,
    max_fee_per_gas_wei: U256,
    eth_usd_e8: U256,
) -> Result<U256> {
    ensure!(gas_limit > 0, "missing gas limit");
    ensure!(!max_fee_per_gas_wei.is_zero(), "missing EIP-1559 max fee");
    ensure!(!eth_usd_e8.is_zero(), "missing verified ETH/USD rate");
    let fee_wei = U256::from(gas_limit)
        .checked_mul(max_fee_per_gas_wei)
        .context("gas multiplication overflow")?;
    let scaled = fee_wei
        .checked_mul(eth_usd_e8)
        .and_then(|n| n.checked_mul(U256::from(110)))
        .context("USD conversion overflow")?;
    let denominator = U256::from(ETH_WEI) * U256::from(100);
    let rounded_up = scaled
        .checked_add(denominator - U256::from(1))
        .context("USD rounding overflow")?
        / denominator;
    Ok(rounded_up)
}

/// Pre-reservation assessment. NEVER consumes an attempt. A live coordinator
/// must complete all expensive checks (including exact sender simulation) before
/// acquiring the single-use reservation; this only checks supplied evidence.
pub fn assess_before_reservation(check: &OneShotCheck) -> Result<U256> {
    ensure!(check.chain_id == 1, "only Ethereum mainnet is supported");
    ensure!(
        check.price_age_seconds <= MAX_PRICE_AGE_SECONDS,
        "ETH/USD quote is stale"
    );
    ensure!(
        check.exact_sender_simulation_succeeded,
        "exact transaction simulation missing or failed"
    );
    ensure!(
        check.nonce_and_contract_verified,
        "nonce, owner, pause or contract identity unverified"
    );
    ensure!(
        check.onchain_pair_and_loan_caps_verified,
        "pair/loan cap unverified"
    );
    ensure!(
        check.route_profitable_after_all_costs,
        "positive executable profit unverified"
    );

    let worst_usd_e8 =
        maximum_gas_cost_usd_e8(check.gas_limit, check.max_fee_per_gas_wei, check.eth_usd_e8)?;
    ensure!(
        worst_usd_e8 <= U256::from(MAX_TOTAL_GAS_USD_E8),
        "worst-case fee exceeds $5 total gas budget"
    );
    Ok(worst_usd_e8)
}

/// Final post-commit gate: caller must supply proof that the *exact* candidate
/// (calldata hash, sender, nonce and fee cap) has been atomically reserved as the
/// one and only durable row, and that its commit succeeded. A boolean is never
/// acceptable evidence from an untrusted caller; the execution coordinator must
/// compare the row returned by an independent DB read against these values.
///
/// Fail CLOSED for ambiguous DB commits or mismatches, do not retry.
pub fn assess_after_reservation(
    check: &OneShotCheck,
    reservation_committed_and_exact: bool,
    persisted_attempt_count: u64,
) -> Result<U256> {
    ensure!(
        reservation_committed_and_exact,
        "committed exact durable reservation missing"
    );
    ensure!(
        persisted_attempt_count == 1,
        "one-shot reservation must be the sole persistent attempt"
    );
    assess_before_reservation(check)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permitted() -> OneShotCheck {
        OneShotCheck {
            chain_id: 1,
            gas_limit: 100_000,
            max_fee_per_gas_wei: U256::from(10_000_000_000u64),
            eth_usd_e8: U256::from(300_000_000_000u64),
            price_age_seconds: 5,
            exact_sender_simulation_succeeded: true,
            nonce_and_contract_verified: true,
            onchain_pair_and_loan_caps_verified: true,
            route_profitable_after_all_costs: true,
        }
    }

    #[test]
    fn accepts_less_than_five_dollars_with_buffer() {
        // 100,000 gas × 10 gwei = .001 ETH = $3; +10% buffer = $3.30.
        assert_eq!(
            assess_before_reservation(&permitted()).unwrap(),
            U256::from(330_000_000u64)
        );
    }

    #[test]
    fn rejects_gas_over_five_dollars() {
        let mut check = permitted();
        check.max_fee_per_gas_wei = U256::from(20_000_000_000u64);
        assert!(assess_before_reservation(&check).is_err());
    }

    #[test]
    fn checks_before_reserving_then_requires_exact_committed_row() {
        let check = permitted();
        // Preflight must not consume a one-shot quota.
        assert!(assess_before_reservation(&check).is_ok());
        // Submission blocked without committed, exact durable evidence.
        assert!(assess_after_reservation(&check, false, 0).is_err());
        assert!(assess_after_reservation(&check, true, 0).is_err());
        assert!(assess_after_reservation(&check, true, 2).is_err());
        assert!(assess_after_reservation(&check, true, 1).is_ok());
    }

    #[test]
    fn rejects_stale_price_and_missing_simulation() {
        let mut check = permitted();
        check.price_age_seconds = 31;
        assert!(assess_before_reservation(&check).is_err());
        check.price_age_seconds = 5;
        check.exact_sender_simulation_succeeded = false;
        assert!(assess_before_reservation(&check).is_err());
    }

    #[test]
    fn rejects_unverified_or_unprofitable_routes() {
        let mut check = permitted();
        check.route_profitable_after_all_costs = false;
        assert!(assess_before_reservation(&check).is_err());
        check.route_profitable_after_all_costs = true;
        check.nonce_and_contract_verified = false;
        assert!(assess_before_reservation(&check).is_err());
        check.nonce_and_contract_verified = true;
        check.onchain_pair_and_loan_caps_verified = false;
        assert!(assess_before_reservation(&check).is_err());
    }

    #[test]
    fn cannot_understate_fractional_usd_cost() {
        let quote =
            maximum_gas_cost_usd_e8(1, U256::from(1), U256::from(300_000_000_000u64)).unwrap();
        assert_eq!(quote, U256::from(1));
    }

    #[test]
    fn committed_reservation_cannot_bypass_profit_or_price_checks() {
        let mut check = permitted();
        check.route_profitable_after_all_costs = false;
        assert!(assess_after_reservation(&check, true, 1).is_err());
        check.route_profitable_after_all_costs = true;
        check.price_age_seconds = 31;
        assert!(assess_after_reservation(&check, true, 1).is_err());
    }

    #[test]
    fn rejects_overflow_and_wrong_chain() {
        assert!(maximum_gas_cost_usd_e8(u64::MAX, U256::MAX, U256::MAX).is_err());
        let mut check = permitted();
        check.chain_id = 8453;
        assert!(assess_before_reservation(&check).is_err());
    }
}
