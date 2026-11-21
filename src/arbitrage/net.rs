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
        let mut probe_amounts = v2_weth_probe_amounts(loan_cap);
        // Pool-specific optimization complements the legacy sizes: short-lived
        // V2/V2 spreads can peak between fixed sample amounts. No new RPCs.
        // All candidates are still checked by the SAME conservative net and
        // execution preflight gates; this is NOT a transaction.
        for amount in optimal_two_hop_v2_sizes(&verified, loan_cap) {
            if !probe_amounts.contains(&amount) {
                probe_amounts.push(amount);
            }
        }
        for amount in probe_amounts {
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

/// Find a bounded, approximately gross-profit-maximizing WETH loan for each
/// *ordered* pair of compatible V2 pools trading the same intermediate token.
/// This is a discovery-only local arithmetic filter (no eth_call, no signer).
/// The net/pricing/preflight checks remain the authority for eligibility.
///
/// A two-leg constant-product route's signed gross profit is unimodal with
/// respect to input amount (ignoring integer rounding). A bounded integer
/// ternary search suffices to pick one extra promising size, without adding
/// a dependency or guessing loan sizes. Exact output uses PoolState.quote.
fn optimal_two_hop_v2_sizes(cache: &ReserveCache, loan_cap: U256) -> Vec<U256> {
    if loan_cap.is_zero() {
        return Vec::new();
    }
    let mut by_token: HashMap<Address, Vec<&super::state::PoolState>> = HashMap::new();
    for pool in cache.values() {
        let other = if pool.token0 == WETH {
            pool.token1
        } else if pool.token1 == WETH {
            pool.token0
        } else {
            continue;
        };
        if other != WETH && !pool.reserve0.is_zero() && !pool.reserve1.is_zero() {
            by_token.entry(other).or_default().push(pool);
        }
    }
    let mut best_sizes = Vec::new();
    for (other, pools) in by_token {
        for (i, first) in pools.iter().enumerate() {
            for (j, second) in pools.iter().enumerate() {
                if i == j || first.pair == second.pair {
                    continue;
                }
                // Both arms must be the same ordered WETH→token→WETH cycle.
                let output = |amount: U256| -> Option<U256> {
                    let (token, mid) = first.quote(WETH, amount)?;
                    if token != other { return None; }
                    let (end, back) = second.quote(other, mid)?;
                    (end == WETH).then_some(back)
                };
                let mut lo = U256::from(1u64);
                let mut hi = loan_cap;
                // Fixed iteration bound; no RPC or unbounded search.
                for _ in 0..128 {
                    if hi <= lo + U256::from(4u64) { break; }
                    let step = (hi - lo) / U256::from(3u64);
                    if step.is_zero() { break; }
                    let a = lo + step;
                    let b = hi - step;
                    let oa = output(a).unwrap_or_default();
                    let ob = output(b).unwrap_or_default();
                    // Compare signed gross profit without constructing a signed U256.
                    // oa-a vs ob-b  <=>  oa+b vs ob+a.
                    let left = oa.checked_add(b);
                    let right = ob.checked_add(a);
                    match (left, right) {
                        (Some(l), Some(r)) if l < r => lo = a,
                        (Some(l), Some(r)) if l > r => hi = b,
                        (Some(_), Some(_)) => { lo = a; hi = b; },
                        _ => { break; } // fail closed on numerical overflow
                    }
                }
                // Integer rounding can shift the argmax a few wei.
                let mut winner = None;
                let mut best = U256::ZERO;
                let mut candidate = lo;
                for _ in 0..=8 {
                    if candidate > hi || candidate > loan_cap { break; }
                    if let Some(out) = output(candidate) {
                        let gross = out.saturating_sub(candidate);
                        if gross > best {
                            best = gross;
                            winner = Some(candidate);
                        }
                    }
                    candidate += U256::from(1u64);
                }
                if let Some(amount) = winner {
                    if !best_sizes.contains(&amount) {
                        best_sizes.push(amount);
                    }
                }
            }
        }
    }
    best_sizes
}

/// Largest fee-excluded, marginal WETH/token spot-price gap between two
/// *canonical and execution-compatible* V2 pools for the same token.
/// The ratio is reserve-based and dimensionless (token decimals cancel).
/// It is a trigger to recheck pinned state, never a profit estimate.
pub fn largest_eligible_weth_v2_gap_bps(
    cache: &ReserveCache,
    canonical_pairs: &HashSet<Address>,
    min_weth: U256,
) -> Option<u64> {
    let mut by_token: HashMap<Address, Vec<(U256, U256)>> = HashMap::new();
    for pool in cache.values() {
        if !canonical_pairs.contains(&pool.pair)
            || !supports_legacy_v2_pool(pool)
            || pool.reserve0.is_zero()
            || pool.reserve1.is_zero()
        {
            continue;
        }
        let (other, weth_reserve, token_reserve) = if pool.token0 == WETH {
            (pool.token1, pool.reserve0, pool.reserve1)
        } else if pool.token1 == WETH {
            (pool.token0, pool.reserve1, pool.reserve0)
        } else {
            continue;
        };
        if other == WETH || weth_reserve < min_weth {
            continue;
        }
        by_token.entry(other).or_default().push((weth_reserve, token_reserve));
    }

    let mut best: Option<u64> = None;
    for pools in by_token.values() {
        for i in 0..pools.len() {
            for j in (i + 1)..pools.len() {
                let (wa, ta) = pools[i];
                let (wb, tb) = pools[j];
                // For uint112 canonical reserves, products fit in 224 bits.
                // checked arithmetic also fails closed for unexpected input.
                let (Some(left), Some(right)) =
                    (ta.checked_mul(wb), tb.checked_mul(wa))
                else { continue };
                let lower = left.min(right);
                if lower.is_zero() { continue; }
                let spread = left.max(right) - lower;
                // 10,000 basis points per 100%; clamp an extreme gap rather
                // than overflow u64. A ratio is NOT an executable trade quote.
                let bps = match spread.checked_mul(U256::from(10_000u64)) {
                    Some(scaled) => u64::try_from(scaled / lower).unwrap_or(u64::MAX),
                    None => u64::MAX,
                };
                best = Some(best.unwrap_or(0).max(bps));
            }
        }
    }
    best
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
    fn cross_pool_gap_reacts_to_reserve_price_dislocation() {
        use super::super::state::PoolState;
        let token = address!("0000000000000000000000000000000000000101");
        let pair_a = address!("0000000000000000000000000000000000000201");
        let pair_b = address!("0000000000000000000000000000000000000202");
        let mut cache = ReserveCache::default();
        cache.insert(PoolState {
            pair: pair_a, token0: WETH, token1: token,
            reserve0: U256::from(10_000u64), reserve1: U256::from(20_000u64),
            fee_bps: 30, block: 100,
        });
        cache.insert(PoolState {
            pair: pair_b, token0: token, token1: WETH,
            reserve0: U256::from(20_000u64), reserve1: U256::from(10_000u64),
            fee_bps: 30, block: 100,
        });
        let all = HashSet::from([pair_a, pair_b]);
        assert_eq!(largest_eligible_weth_v2_gap_bps(&cache, &all,
            U256::from(1u64)), Some(0));
        // A real Sync reserve change shifts the price 1%, over 65 bps.
        cache.get_mut(&pair_a).unwrap().reserve1 = U256::from(20_200u64);
        assert_eq!(largest_eligible_weth_v2_gap_bps(&cache, &all,
            U256::from(1u64)), Some(100));
        assert_eq!(largest_eligible_weth_v2_gap_bps(&cache,
            &HashSet::from([pair_a]), U256::from(1u64)), None);
        assert_eq!(largest_eligible_weth_v2_gap_bps(&cache, &all,
            U256::from(11_000u64)), None);
        cache.get_mut(&pair_a).unwrap().fee_bps = 25;
        assert_eq!(largest_eligible_weth_v2_gap_bps(&cache, &all,
            U256::from(1u64)), None);
    }

    #[test]
    fn optimizer_finds_real_reserve_based_positive_size_within_cap() {
        use super::super::state::PoolState;
        let token = address!("0000000000000000000000000000000000000101");
        let mut cache = ReserveCache::default();
        cache.insert(PoolState {
            pair: address!("0000000000000000000000000000000000000201"),
            token0: WETH, token1: token,
            reserve0: U256::from(1_000_000u64),
            reserve1: U256::from(2_000_000u64),
            fee_bps: 30, block: 10,
        });
        cache.insert(PoolState {
            pair: address!("0000000000000000000000000000000000000202"),
            token0: token, token1: WETH,
            reserve0: U256::from(1_800_000u64),
            reserve1: U256::from(1_000_000u64),
            fee_bps: 30, block: 10,
        });
        let cap = U256::from(100_000u64);
        let optimized = optimal_two_hop_v2_sizes(&cache, cap);
        assert!(!optimized.is_empty(), "dislocation should have an optimum");
        assert!(optimized.iter().all(|x| !x.is_zero() && *x <= cap));
        let first = cache.get(&address!("0000000000000000000000000000000000000201")).unwrap();
        let second = cache.get(&address!("0000000000000000000000000000000000000202")).unwrap();
        assert!(optimized.iter().any(|&amount| {
            let (_, mid) = first.quote(WETH, amount).unwrap();
            let (_, output) = second.quote(token, mid).unwrap();
            output > amount
        }));
        assert!(optimal_two_hop_v2_sizes(&cache, U256::ZERO).is_empty());
    }

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
