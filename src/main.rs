use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use alloy::transports::ws::WsConnect;
use sandooo::arbitrage::{
    find_cycles,
    rpc::{load_allowlisted_reserves, load_token_decimals, verify_allowlisted_tokens},
    state::sync_topic,
    topology::count_closed_routes,
};
use sandooo::common::{constants::get_env, pools::load_all_pools, streams::*};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, Notify, RwLock};
use tokio::task::JoinSet;
use tracing::{info, warn};
use anyhow::Context;
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let env = Arc::new(get_env()?);
    let ws = WsConnect::new(env.wss_url.clone());
    let provider = ProviderBuilder::new().connect_ws(ws).await?;

    let scan_enabled = std::env::var("POOL_SCAN_ENABLED")
        .map(|v| !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true);
    if !scan_enabled {
        return run_allowlisted_simulation(provider).await;
    }

    let pools = Arc::new(load_all_pools(&provider).await?);
    info!(pools = pools.len(), "pool cache ready");

    let (tx, _) = broadcast::channel(512);
    let head = Arc::new(Mutex::new(provider.get_block_number().await?));

    let mut set = JoinSet::new();
    set.spawn(stream_new_blocks(provider.clone(), tx.clone()));
    set.spawn(stream_pending_transactions(provider.clone(), tx.clone()));
    set.spawn(sandooo::sandwich::strategy::run_sandwich_strategy(
        env, provider, pools, tx.subscribe(), head,
    ));
    while let Some(r) = set.join_next().await { r??; }
    Ok(())
}

fn pow10(decimals: u8) -> alloy::primitives::U256 {
    let mut result = alloy::primitives::U256::from(1u64);
    for _ in 0..decimals {
        result *= alloy::primitives::U256::from(10u64);
    }
    result
}

async fn run_allowlisted_simulation<P>(provider: P) -> anyhow::Result<()>
where
    P: Provider<alloy::network::Ethereum> + Clone + Send + Sync + 'static,
{
    let allowlist = std::env::var("POOL_ALLOWLIST").unwrap_or_default();
    if allowlist.trim().is_empty() {
        warn!("POOL_ALLOWLIST is empty; worker will remain idle");
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    }

    // Auto-sized discovery probes are the default even when legacy SIM_AMOUNT_RAW is set.
    let amount_mode = std::env::var("SIM_AMOUNT_MODE")
        .unwrap_or_else(|_| "auto".to_string())
        .to_ascii_lowercase();
    let sim_amount_override = match amount_mode.as_str() {
        "auto" => None,
        "fixed" => {
            let amount = std::env::var("SIM_AMOUNT_RAW")
                .context("SIM_AMOUNT_RAW is required for SIM_AMOUNT_MODE=fixed")?
                .parse::<u64>()
                .context("SIM_AMOUNT_RAW must be a positive raw integer")?;
            anyhow::ensure!(amount > 0, "SIM_AMOUNT_RAW must be positive");
            Some(amount)
        }
        other => anyhow::bail!("SIM_AMOUNT_MODE must be auto or fixed, got {other}"),
    };
    let sim_max_hops = std::env::var("SIM_MAX_HOPS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(3);
    let sim_min_profit = std::env::var("SIM_MIN_PROFIT_RAW")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1);
    let sim_interval = std::env::var("SIM_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(15)
        .max(1);
    let premium_bps = std::env::var("SIM_AAVE_PREMIUM_BPS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5);
    let gas_cost_raw = std::env::var("SIM_GAS_COST_RAW")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let near_miss_count = std::env::var("SIM_NEAR_MISS_LOG")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(3);
    let stale_blocks = std::env::var("SIM_STALE_BLOCKS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);

    let block = provider.get_block_number().await?;
    let initial = load_allowlisted_reserves(&provider, &allowlist, block).await?;
    // Prevent impossible/incorrect pool routes from silently passing discovery.
    verify_allowlisted_tokens(&provider, &initial).await?;
    let structural_routes = count_closed_routes(&initial, sim_max_hops);
    if structural_routes == 0 {
        warn!(pools = initial.len(), max_hops = sim_max_hops,
            "configured allowlist contains NO closed arbitrage routes");
    } else {
        info!(structural_routes, "configured allowlist contains structural routes (profit not proven)");
    }
    let pair_addresses: Vec<_> = initial.pairs().copied().collect();
    let unique_tokens: Vec<_> = initial
        .values()
        .flat_map(|p| [p.token0, p.token1])
        .chain(initial.v3_values().flat_map(|p| [p.token0, p.token1]))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let decimals_map = load_token_decimals(&provider, unique_tokens).await;
    let cache = Arc::new(RwLock::new(initial));
    let sync_notify = Arc::new(Notify::new());

    // Event-driven reserve updates from V2 Sync logs (best-effort).
    // V3 pools still use periodic reconciliation until a verified swap-state
    // subscription is implemented.
    let sync_cache = cache.clone();
    let sync_provider = provider.clone();
    let sync_pairs = pair_addresses.clone();
    let event_notify = sync_notify.clone();
    tokio::spawn(async move {
        let filter = Filter::new().address(sync_pairs).event_signature(sync_topic());
        match sync_provider.subscribe_logs(&filter).await {
            Ok(sub) => {
                let mut stream = sub.into_stream();
                while let Some(log) = stream.next().await {
                    let pair = log.address();
                    let block = log.block_number.unwrap_or_default();
                    let data: &[u8] = log.data().data.as_ref();
                    let mut guard = sync_cache.write().await;
                    if guard.apply_sync_log(pair, data, block) {
                        event_notify.notify_one();
                    }
                }
                warn!("sync log stream ended");
            }
            Err(error) => warn!(%error, "sync log subscription unavailable; falling back to polling refresh"),
        }
    });

    // Timer is independent of Sync events so reconciliation still happens
    // even if the pool is quiet or the log stream is interrupted.
    let mut refresh_timer = tokio::time::interval(std::time::Duration::from_secs(sim_interval));
    refresh_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    refresh_timer.tick().await; // We already fetched the initial snapshot.
    let mut refresh_due = false;
    let mut observed_block = block;
    info!(
        amount_mode = %amount_mode,
        interval_secs = sim_interval,
        v2_event_driven = true,
        v3_event_driven = false,
        "arbitrage discovery scan ready"
    );

    loop {
        if refresh_due {
            refresh_due = false;
            match provider.get_block_number().await {
                Ok(b) => {
                    observed_block = observed_block.max(b);
                    match load_allowlisted_reserves(&provider, &allowlist, b).await {
                        Ok(fresh) => {
                            let mut guard = cache.write().await;
                            if fresh.last_block() >= guard.last_block() {
                                *guard = fresh;
                            }
                        }
                        Err(error) => warn!(%error, "allowlisted reserve refresh failed"),
                    }
                }
                Err(error) => warn!(%error, "failed to fetch block number"),
            }
        }
        let block = observed_block.max(cache.read().await.last_block());
        observed_block = block;

        let (pools, fresh_count, stale_count, cycles, near_misses, last_block, amounts_tried) = {
            let guard = cache.read().await;
            // Count fresh vs stale pools
            let fresh_count = guard.values()
                .filter(|p| block.saturating_sub(p.block) <= stale_blocks)
                .count()
                + guard.v3_values()
                    .filter(|p| block.saturating_sub(p.block) <= stale_blocks)
                    .count();
            let stale_count = guard.len() - fresh_count;

            let mut by_decimals: HashMap<u8, Vec<alloy::primitives::Address>> = HashMap::new();
            let mut seen: HashSet<alloy::primitives::Address> = HashSet::new();
            for pool in guard.values() {
                if block.saturating_sub(pool.block) > stale_blocks {
                    continue;
                }
                for token in [pool.token0, pool.token1] {
                    if seen.insert(token) {
                        let d = *decimals_map.get(&token).unwrap_or(&18u8);
                        if d <= 30 {
                            by_decimals.entry(d).or_default().push(token);
                        } else {
                            warn!(decimals = d, token = %token, "unsupported token decimals, skipping probe");
                        }
                    }
                }
            }
            for pool in guard.v3_values() {
                if block.saturating_sub(pool.block) > stale_blocks {
                    continue;
                }
                for token in [pool.token0, pool.token1] {
                    if seen.insert(token) {
                        let d = *decimals_map.get(&token).unwrap_or(&18u8);
                        if d <= 30 {
                            by_decimals.entry(d).or_default().push(token);
                        } else {
                            warn!(decimals = d, token = %token, "unsupported token decimals, skipping probe");
                        }
                    }
                }
            }

            let min_net = alloy::primitives::U256::from(sim_min_profit);
            let mut all_cycles = Vec::new();
            let mut near_miss_list: Vec<(alloy::primitives::U256, usize)> = Vec::new();
            let mut amounts_tried = 0usize;

            // Build amount list
            let run_amounts: Vec<(alloy::primitives::U256, Vec<alloy::primitives::Address>)> =
                if let Some(amount) = sim_amount_override {
                    let starts: Vec<_> = seen.iter().copied().collect();
                    vec![(alloy::primitives::U256::from(amount), starts)]
                } else {
                    let mut runs = Vec::new();
                    for (decimals, starts) in &by_decimals {
                        let unit = pow10(*decimals);
                        let factors: &[u64] = if *decimals <= 6 {
                            &[100, 1_000, 10_000, 50_000]
                        } else {
                            &[1, 5, 10, 50]
                        };
                        for &f in factors {
                            let amount = if *decimals >= 18 {
                                if f == 1 {
                                    unit / alloy::primitives::U256::from(10u64)
                                } else {
                                    alloy::primitives::U256::from(f) * unit / alloy::primitives::U256::from(10u64)
                                }
                            } else {
                                alloy::primitives::U256::from(f) * unit
                            };
                            if amount > alloy::primitives::U256::ZERO {
                                runs.push((amount, starts.clone()));
                            }
                        }
                        // Reserve-proportional: 0.01%, 0.1%, 1% of smallest reserve
                        for pool in guard.values() {
                            for (token, reserve) in [(pool.token0, pool.reserve0), (pool.token1, pool.reserve1)] {
                                if starts.contains(&token) && !reserve.is_zero() {
                                    for pct_num in [1u64, 10, 100] {
                                        let amt = reserve * alloy::primitives::U256::from(pct_num)
                                            / alloy::primitives::U256::from(10_000u64);
                                        if amt > alloy::primitives::U256::ZERO {
                                            runs.push((amt, vec![token]));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    runs
                };

            for (amount, starts) in run_amounts {
                amounts_tried += 1;
                let found = find_cycles(
                    &guard,
                    &starts,
                    amount,
                    sim_max_hops,
                    alloy::primitives::U256::ZERO,
                );
                // Only compare amounts in the SAME loan asset. SIM_GAS_COST_RAW
                // is wei of native ETH; the first hop's reserve ratio is NOT a
                // reliable ETH-to-start-token oracle (especially for V3/WETH).
                // Do not authorize spending or call these post-gas net profits.
                // Actual submission requires the separate strict preflight,
                // fresh on-chain call simulation, and independently priced gas.
                let premium = amount * alloy::primitives::U256::from(premium_bps)
                    / alloy::primitives::U256::from(10_000u64);
                let mut kept = Vec::new();
                for c in found {
                    if c.gross_profit > premium
                        && c.gross_profit - premium >= min_net
                    {
                        kept.push(c);
                    } else if c.gross_profit > alloy::primitives::U256::ZERO {
                        near_miss_list.push((c.gross_profit, c.hops.len()));
                    }
                }
                all_cycles.extend(kept);
            }

            // Dedup: keep best gross_profit per (pairs set, hops)
            let mut seen_keys: HashSet<String> = HashSet::new();
            let mut deduped = Vec::new();
            for c in &all_cycles {
                let mut pairs: Vec<_> = c.hops.iter().map(|h| format!("{:?}", h.pair)).collect();
                pairs.sort();
                let key = format!("{}:{}", pairs.join(","), c.hops.len());
                if seen_keys.insert(key) {
                    deduped.push(c.clone());
                }
            }

            // Sort near-misses, keep top N
            near_miss_list.sort_by(|a, b| b.0.cmp(&a.0));
            near_miss_list.truncate(near_miss_count);

            (guard.len(), fresh_count, stale_count, deduped, near_miss_list, guard.last_block(), amounts_tried)
        };

        info!(
            pools,
            fresh = fresh_count,
            stale = stale_count,
            cycles = cycles.len(),
            near_misses = near_misses.len(),
            block = last_block,
            amounts_tried,
            max_hops = sim_max_hops,
            premium_bps,
            gas_cost_raw,
            "arbitrage discovery (gross candidates only, not live-trade signals)"
        );

        for cycle in cycles.iter().take(5) {
            // Aave premium is in loan asset raw units. This is intentionally
            // PRE-GAS only: real net profit has not been independently verified.
            let premium = cycle.amount_in * alloy::primitives::U256::from(premium_bps)
                / alloy::primitives::U256::from(10_000u64);
            let margin_before_gas = cycle.gross_profit.saturating_sub(premium);
            let v2_contract_compatible = {
                let guard = cache.read().await;
                cycle.hops.iter().all(|hop| guard.get(&hop.pair)
                    .is_some_and(|pool| pool.fee_bps == 30))
            };
            info!(
                gross_profit_raw = %cycle.gross_profit,
                margin_after_premium_before_gas_raw = %margin_before_gas,
                net_profit_verified = false,
                v2_contract_compatible,
                configured_gas_cost_wei_unverified = gas_cost_raw,
                amount_in = %cycle.amount_in,
                amount_out = %cycle.amount_out,
                start_asset = %cycle.start,
                hops = cycle.hops.len(),
                freshest_block = cycle.freshest_block,
                "gross discovery candidate — NOT approved for execution"
            );
        }

        // Log near-misses to help understand why cycles=0
        if cycles.is_empty() && !near_misses.is_empty() {
            for (gp, hops) in &near_misses {
                info!(
                    gross_profit = %gp,
                    hops,
                    "near-miss cycle (did not clear net-profit filter)"
                );
            }
        }

        // Debug: log per-pool reserves
        if std::env::var("DEBUG").map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(false) {
            for pool in cache.read().await.values() {
                let age = block.saturating_sub(pool.block);
                info!(
                    pair = ?pool.pair,
                    fee_bps = pool.fee_bps,
                    reserve0 = %pool.reserve0,
                    reserve1 = %pool.reserve1,
                    age_blocks = age,
                    "pool reserve"
                );
            }
        }

        tokio::select! {
            biased;
            _ = refresh_timer.tick() => refresh_due = true,
            _ = sync_notify.notified() => {
                // Coalesce bursts to avoid repeated CPU-heavy calculations.
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            }
        }
    }
}
#[cfg(test)]
mod scanner_tests {
    use super::pow10;
    use alloy::primitives::U256;

    #[test]
    fn six_decimal_amount_units() {
        assert_eq!(pow10(6), U256::from(1_000_000u64));
    }

    #[test]
    fn reported_candidate_is_not_after_gas_profit() {
        // 100 USDC borrowed, 100.122318 USDC returned (6 decimals).
        // 5 bps flash-loan premium = 0.05 USDC; 0.072318 USDC
        // remains BEFORE ETH gas and any failed-transaction costs.
        let amount = U256::from(100_000_000u64);
        let gross = U256::from(122_318u64);
        let premium = amount * U256::from(5u64) / U256::from(10_000u64);
        assert_eq!(premium, U256::from(50_000u64));
        assert_eq!(gross - premium, U256::from(72_318u64));
    }

    #[test]
    fn eighteen_decimal_amount_units() {
        assert_eq!(pow10(18), U256::from(1_000_000_000_000_000_000u64));
    }
}
