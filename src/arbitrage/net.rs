use super::{
    find_cycles,
    preflight::{prepare_v2_flash_loan, supports_legacy_v2_pool, ExecutionLimits, GasQuote},
    ReserveCache,
};
use crate::common::constants::WETH;
use alloy::{
    network::Ethereum,
    primitives::{address, Address, U256},
    providers::Provider,
    sol,
};
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use tracing::info;

sol! {
    #[sol(rpc)]
    interface AavePremium {
        function FLASHLOAN_PREMIUM_TOTAL() external view returns (uint128);
    }
}

pub struct NetScan {
    pub gas_units: u64,
    pub tip_wei: u64,
    pub slippage_bps: u64,
    pub min_weth: U256,
    pub pairs: HashSet<Address>,
}

impl NetScan {
    pub async fn report<P: Provider<Ethereum> + Clone>(
        &self,
        provider: &P,
        snapshot: &ReserveCache,
        block: u64,
        min_profit: u64,
        max_hops: usize,
    ) -> Result<()> {
        let mut verified = ReserveCache::default();
        for pool in snapshot.values() {
            let weth_reserve = if pool.token0 == WETH {
                pool.reserve0
            } else {
                pool.reserve1
            };
            if self.pairs.contains(&pool.pair)
                && supports_legacy_v2_pool(pool)
                && pool.block == block
                && weth_reserve >= self.min_weth
                && !pool.reserve0.is_zero()
                && !pool.reserve1.is_zero()
            {
                verified.insert(pool.clone());
            }
        }
        let header = provider
            .get_block_by_number(block.into())
            .await?
            .context("missing block")?;
        let base_fee = header
            .header
            .base_fee_per_gas
            .context("missing Ethereum base fee")?;
        let max_fee_per_gas_wei = U256::from(base_fee) * U256::from(2)
            + U256::from(self.tip_wei);
        let gas = U256::from(self.gas_units) * max_fee_per_gas_wei;
        let aave = AavePremium::new(
            address!("87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"),
            provider.clone(),
        );
        let premium_bps = u64::try_from(
            aave.FLASHLOAN_PREMIUM_TOTAL()
                .block(block.into())
                .call()
                .await?,
        )?;
        anyhow::ensure!(
            (1..=1_000).contains(&premium_bps) && !gas.is_zero(),
            "invalid Aave premium or gas"
        );
        let loan_cap = self.min_weth / U256::from(100);
        let limits = ExecutionLimits {
            current_block: block,
            max_pool_age_blocks: 0,
            max_hops: max_hops.min(3),
            max_flash_loan: loan_cap,
            max_gas_spend_wei: gas,
            premium_bps,
            min_profit_after_gas_in_asset: U256::from(min_profit.max(1)),
        };
        let cost = GasQuote {
            gas_spend_wei: gas,
            gas_cost_in_asset: gas,
            quoted_at_block: block,
        };
        let mut positive = HashMap::new();
        // Diagnostics only: do not change the net profit or preflight gates.
        // This helps distinguish "no compatible cycles" from "too little
        // margin after gas" without logging secrets or candidate calldata.
        let mut v2_gross_cycles = 0u64;
        let mut rejected_by_costs = 0u64;
        let mut rejected_by_net_floor = 0u64;
        let mut rejected_by_preflight = 0u64;
        let mut accepted_probe_sizes = 0u64;
        // Use a bounded range of loan sizes; a profitable 0.06-0.10 WETH
        // cycle can be missed by only probing 0.25, 0.025 and 0.0025 WETH.
        // All probes remain <= the existing max_flash_loan; this is
        // discovery-only and does NOT authorize execution or change gas.
        for amount in v2_weth_probe_amounts(loan_cap) {
            for cycle in find_cycles(&verified, &[WETH], amount, limits.max_hops, U256::ZERO) {
                v2_gross_cycles += 1;
                let Some(net) = conservative_net(
                    cycle.amount_out,
                    amount,
                    premium_bps,
                    gas,
                    self.slippage_bps,
                ) else {
                    rejected_by_costs += 1;
                    continue;
                };
                if net < limits.min_profit_after_gas_in_asset {
                    rejected_by_net_floor += 1;
                    continue;
                }
                if prepare_v2_flash_loan(&verified, &cycle, &limits, Some(&cost)).is_err() {
                    rejected_by_preflight += 1;
                    continue;
                }
                accepted_probe_sizes += 1;
                let route: Vec<_> = cycle.hops.iter().map(|hop| hop.pair).collect();
                let best = positive.entry(route).or_insert((net, amount));
                if net > best.0 {
                    *best = (net, amount);
                }
            }
        }
        // Counterfactual USD ceilings, sampled on the same cadence as the
        // existing Chainlink diagnostic. This DOES NOT change the actual
        // $5 one-shot sender guard, current preflight, gas quote or route set.
        // Tier counts are subsets of routes already positive after estimated
        // gas/slippage/Aave premium and accepted by the read-only preflight.
        // They are NOT confirmed executable trades or proof of missed profits.
        if block % 25 == 0 {
            match super::gas_usd_diagnostic::read_gas_usd_diagnostic(
                provider, block, self.gas_units, max_fee_per_gas_wei,
                header.header.timestamp,
            ).await {
                Ok((max_usd_e8, quote_age_seconds, within_five_usd)) => {
                    let counts = usd_budget_tier_counts(max_usd_e8, positive.len());
                    info!(block, max_usd_e8 = %max_usd_e8, quote_age_seconds,
                        within_five_usd, enforced_on_sender = false,
                        "read-only Ethereum ETH/USD gas ceiling (not trading approval)");
                    info!(block, max_usd_e8 = %max_usd_e8,
                        routes_within_5_usd = counts[0],
                        routes_within_10_usd = counts[1],
                        routes_within_15_usd = counts[2],
                        routes_within_20_usd = counts[3],
                        additional_routes_5_to_10_usd = counts[1] - counts[0],
                        additional_routes_10_to_15_usd = counts[2] - counts[1],
                        additional_routes_15_to_20_usd = counts[3] - counts[2],
                        positive_after_estimated_costs = positive.len(),
                        not_execution_approval = true,
                        "read-only $5/$10/$15/$20 counterfactual gas ceilings");
                }
                Err(error) => tracing::warn!(block, %error,
                    "read-only budget tiers unavailable; no unverified USD comparison"),
            }
        }
        info!(block, verified_v2_pools = verified.len(), positive_net_estimates = positive.len(),
            v2_gross_cycles, rejected_by_costs, rejected_by_net_floor,
            rejected_by_preflight, accepted_probe_sizes,
            gas_units_budget = self.gas_units, gas_budget_wei = %gas, premium_bps,
            slippage_bps = self.slippage_bps, "read-only WETH V2 net screen");
        for (route, (net, amount)) in positive {
            info!(block, ?route, loan_wei = %amount, estimated_net_wei = %net,
                "positive estimated net for V2-compatible route; executor simulation pending");
        }
        Ok(())
    }
}

/// Pure counterfactual on the SAME conservative maxFeePerGas quote and
/// previously accepted candidate routes. It does not relax any gas gate.
fn usd_budget_tier_counts(usd_e8: U256, eligible_route_count: usize) -> [usize; 4] {
    [5u64, 10, 15, 20].map(|cap| {
        if usd_e8 <= U256::from(cap) * U256::from(100_000_000u64) {
            eligible_route_count
        } else {
            0
        }
    })
}

/// Sample diverse amounts without exceeding the existing flash-loan cap.
/// Deduplicate small amounts that round to the same integer raw token amount.
fn v2_weth_probe_amounts(loan_cap: U256) -> Vec<U256> {
    let mut amounts = Vec::new();
    for percent in [100u64, 50, 40, 30, 25, 20, 10, 5, 2, 1] {
        let Some(scaled) = loan_cap.checked_mul(U256::from(percent)) else { continue };
        let amount = scaled / U256::from(100u64);
        if !amount.is_zero() && amount <= loan_cap && !amounts.contains(&amount) {
            amounts.push(amount);
        }
    }
    amounts
}

fn conservative_net(
    out: U256,
    loan: U256,
    premium_bps: u64,
    gas: U256,
    slippage_bps: u64,
) -> Option<U256> {
    let bps = U256::from(10_000);
    let buffered = out * (bps - U256::from(slippage_bps)) / bps;
    let premium = (loan * U256::from(premium_bps) + bps - U256::from(1)) / bps;
    buffered
        .checked_sub(loan + premium + gas)
        .filter(|net| !net.is_zero())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usd_tiers_are_monotone_and_never_change_the_sender_cap() {
        assert_eq!(usd_budget_tier_counts(U256::from(499_999_999u64), 3), [3, 3, 3, 3]);
        assert_eq!(usd_budget_tier_counts(U256::from(500_000_001u64), 3), [0, 3, 3, 3]);
        assert_eq!(usd_budget_tier_counts(U256::from(1_000_000_001u64), 2), [0, 0, 2, 2]);
        assert_eq!(usd_budget_tier_counts(U256::from(1_500_000_001u64), 2), [0, 0, 0, 2]);
        assert_eq!(usd_budget_tier_counts(U256::from(2_000_000_001u64), 2), [0, 0, 0, 0]);
        assert_eq!(usd_budget_tier_counts(U256::from(600_000_000u64), 0), [0, 0, 0, 0]);
        assert_eq!(super::super::one_shot::MAX_TOTAL_GAS_USD_E8, 500_000_000);
    }

    #[test]
    fn weth_probe_amounts_cover_middle_sizes_within_existing_cap() {
        let cap = U256::from(250_000u64);
        let amounts = v2_weth_probe_amounts(cap);
        assert!(amounts.contains(&U256::from(100_000u64)));
        assert!(amounts.contains(&U256::from(62_500u64)));
        assert!(amounts.contains(&U256::from(25_000u64)));
        assert!(amounts.iter().all(|&amount| !amount.is_zero() && amount <= cap));
        assert_eq!(amounts.len(), 10);
    }

    #[test]
    fn weth_probe_amounts_deduplicate_and_reject_zero() {
        assert!(v2_weth_probe_amounts(U256::ZERO).is_empty());
        let amounts = v2_weth_probe_amounts(U256::from(1u64));
        assert_eq!(amounts, vec![U256::from(1u64)]);
    }

    #[test]
    fn rejects_gross_profit_that_does_not_cover_buffered_costs() {
        let loan = U256::from(10_001);
        let gas = U256::from(90);
        // The premium rounds up to 6. A 1% buffer reduces 10,200 to 10,098.
        assert_eq!(
            conservative_net(U256::from(10_200), loan, 5, gas, 100),
            Some(U256::from(1))
        );
        assert_eq!(
            conservative_net(U256::from(10_199), loan, 5, gas, 100),
            None
        );
        assert_eq!(
            conservative_net(U256::from(10_150), loan, 5, gas, 100),
            None
        );
    }
}
