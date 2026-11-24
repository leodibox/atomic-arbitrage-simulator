use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use alloy::transports::ws::WsConnect;
use anyhow::Context;
use futures::StreamExt;
use sandooo::arbitrage::{
    find_cycles,
    net::{largest_eligible_weth_v2_gap_bps, NetScan},
    preflight::supports_legacy_v2_pool,
    rpc::{
        discover_weth_v2, discover_weth_v3, load_allowlisted_reserves, load_token_decimals,
        verify_allowlisted_tokens,
    },
    state::sync_topic,
    topology::count_closed_routes,
    Cycle, ReserveCache,
};
use sandooo::common::{constants::get_env, pools::load_all_pools, streams::*};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, Notify, RwLock};
use tokio::task::JoinSet;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let env = Arc::new(get_env()?);
    let ws = WsConnect::new(env.wss_url.clone());
    let provider = ProviderBuilder::new().connect_ws(ws).await?;

    // Only PUBLIC identity and bytecode checks: never read a private key here.
    // Missing/partial configuration cannot become trading authorization.
    match sandooo::arbitrage::guarded_wiring::GuardedPublicConfig::from_environment() {
        Ok(Some(config)) => {
            match sandooo::arbitrage::guarded_wiring::verify_guarded_read_only(&provider, &config)
                .await
            {
                Ok(attestation) => info!(
                    executor = %attestation.executor, owner = %attestation.owner,
                    recipient = %attestation.recipient,
                    code_keccak = %attestation.code_keccak,
                    paused = attestation.paused, auto_payout = attestation.auto_payout,
                    can_sign = false, transaction_submission_enabled = false,
                    "Guarded on-chain identity verified (read-only, NOT live activation)"
                ),
                Err(error) => warn!(%error,
                "Guarded executor read-only attestation FAILED; no execution permitted"),
            }
        }
        Ok(None) => info!(
            transaction_submission_enabled = false,
            "Guarded executor deployment/owner not configured; scanner remains read-only"
        ),
        Err(error) => warn!(%error,
            "Guarded executor public configuration INVALID; no execution permitted"),
    }

    // Fail closed: never select the legacy pending-transaction strategy by

    // omission, typo, or an unknown value.
    let scan_config = std::env::var("POOL_SCAN_ENABLED").ok();
    let scan_enabled = parse_scan_enabled(scan_config.as_deref())?;
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
        env,
        provider,
        pools,
        tx.subscribe(),
        head,
    ));
    while let Some(r) = set.join_next().await {
        r??;
    }
    Ok(())
}

/// Missing configuration selects read-only discovery; only the literal
/// `true` activates the separately configured legacy strategy.
fn parse_scan_enabled(value: Option<&str>) -> anyhow::Result<bool> {
    match value {
        None => Ok(false),
        Some(v) if v.eq_ignore_ascii_case("false") => Ok(false),
        Some(v) if v.eq_ignore_ascii_case("true") => Ok(true),
        _ => anyhow::bail!("POOL_SCAN_ENABLED must be true or false"),
    }
}

/// Retain each distinct start-token + ordered route, including the reverse
/// direction. For multiple probes of the SAME start token and route, preserve
/// the largest after-Aave PRE-GAS margin, not the first arbitrary probe.
/// This is discovery deduplication only, NOT post-gas trade approval.
fn best_discovery_per_ordered_route(cycles: Vec<Cycle>, premium_bps: u64) -> Vec<Cycle> {
    let pre_gas_margin = |cycle: &Cycle| {
        let premium = cycle.amount_in * alloy::primitives::U256::from(premium_bps)
            / alloy::primitives::U256::from(10_000u64);
        cycle.gross_profit.saturating_sub(premium)
    };
    let mut winners: Vec<Cycle> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();
    for candidate in cycles {
        let mut key = format!("{:?}", candidate.start);
        for hop in &candidate.hops {
            key.push_str(&format!(
                "|{:?}:{:?}:{:?}",
                hop.pair, hop.token_in, hop.token_out
            ));
        }
        if let Some(&index) = positions.get(&key) {
            if pre_gas_margin(&candidate) > pre_gas_margin(&winners[index]) {
                winners[index] = candidate;
            }
        } else {
            positions.insert(key, winners.len());
            winners.push(candidate);
        }
    }
    winners
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
    let live = sandooo::arbitrage::live::coordinator::LiveConfig::from_env()
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    let live_signer = match (&live, std::env::var("PRIVATE_KEY")) {
        (Some(_), Ok(key)) => Some(
            <alloy::signers::local::PrivateKeySigner as std::str::FromStr>::from_str(key.trim())
                .map_err(|error| anyhow::anyhow!("invalid PRIVATE_KEY: {error}"))?,
        ),
        (Some(_), Err(_)) => anyhow::bail!("ARB_LIVE_SEND=true requires PRIVATE_KEY"),
        (None, _) => None,
    };
    if live.is_some() {
        info!(live_send = true, "LIVE SEND ENABLED; gas will be spent");
    }
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
    let mut expanded_allowlist = allowlist.clone();
    let mut net_scan = None;
    let mut pending_gas_units = if let Ok(value) = std::env::var("SIM_V2_GAS_UNITS") {
        let gas_units = value.parse::<u64>().context("invalid SIM_V2_GAS_UNITS")?;
        anyhow::ensure!(gas_units > 0, "SIM_V2_GAS_UNITS must be positive");
        Some(gas_units)
    } else {
        None
    };
    let min_weth = alloy::primitives::U256::from(8) * pow10(18);
    let configured_pairs: HashSet<_> = initial
        .pairs()
        .copied()
        .chain(initial.v3_values().map(|pool| pool.pair))
        .collect();
    let structural_routes = count_closed_routes(&initial, sim_max_hops);
    if structural_routes == 0 {
        warn!(
            pools = initial.len(),
            max_hops = sim_max_hops,
            "configured allowlist contains NO closed arbitrage routes"
        );
    } else {
        info!(
            structural_routes,
            "configured allowlist contains structural routes (profit not proven)"
        );
    }
    let pair_addresses: Vec<_> = initial.pairs().copied().collect();
    let unique_tokens: Vec<_> = initial
        .values()
        .flat_map(|p| [p.token0, p.token1])
        .chain(initial.v3_values().flat_map(|p| [p.token0, p.token1]))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let mut decimals_map = load_token_decimals(&provider, unique_tokens).await;
    let cache = Arc::new(RwLock::new(initial));
    let sync_notify = Arc::new(Notify::new());

    // Event-driven reserve updates from V2 Sync logs (best-effort).
    // V3 pools still use periodic reconciliation until a verified swap-state
    // subscription is implemented.
    subscribe_v2_sync(
        provider.clone(),
        pair_addresses,
        cache.clone(),
        sync_notify.clone(),
    );

    // Timer is independent of Sync events so reconciliation still happens
    // even if the pool is quiet or the log stream is interrupted.
    let mut refresh_timer = tokio::time::interval(std::time::Duration::from_secs(sim_interval));
    refresh_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    if pending_gas_units.is_none() {
        refresh_timer.tick().await; // We already fetched the initial snapshot.
    }
    let mut refresh_due = false;
    // V2 Sync events normally trigger gross discovery only. A candidate after
    // an event deserves one fresh, pinned net screen before the next timer tick.
    let mut sync_event_pending = false;
    let mut last_event_net_screen_block: Option<u64> = None;
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
                    let discovery = retry_v2_discovery(
                        &mut pending_gas_units,
                        Duration::from_secs(5),
                        || async {
                            let pairs = discover_weth_v2(&provider, b, min_weth).await?;
                            let v3_floor = std::env::var("ARB_V3_MIN_LIQUIDITY")
                                .ok()
                                .and_then(|v| v.parse::<u128>().ok())
                                .map(alloy::primitives::U256::from)
                                .unwrap_or(alloy::primitives::U256::from(
                                    1_000_000_000_000_000u128,
                                ));
                            let v3_pools = discover_weth_v3(&provider, b, v3_floor).await?;
                            let (expanded, mut added) =
                                expand_v2_allowlist(&allowlist, &configured_pairs, &pairs);
                            let (expanded, v3_added) =
                                expand_v3_allowlist(&expanded, &added, &v3_pools);
                            added.extend(v3_added);
                            let snapshot =
                                load_allowlisted_reserves(&provider, &expanded, b).await?;
                            let new_tokens: Vec<_> = pairs
                                .iter()
                                .filter(|(pair, _, _)| added.contains(pair))
                                .flat_map(|(_, token0, token1)| [*token0, *token1])
                                .chain(
                                    v3_pools
                                        .iter()
                                        .filter(|(pool, _, _, _)| added.contains(pool))
                                        .flat_map(|(_, token0, token1, _)| [*token0, *token1]),
                                )
                                .filter(|token| !decimals_map.contains_key(token))
                                .collect();
                            let decimals = load_token_decimals(&provider, new_tokens).await;
                            Ok((pairs, v3_pools, expanded, added, snapshot, decimals))
                        },
                    )
                    .await;
                    let mut added_pairs = Vec::new();
                    let (refreshed, net_snapshot_available) = if let Some((
                        gas_units,
                        (pairs, v3_pools, expanded, added, snapshot, decimals),
                    )) = discovery
                    {
                        info!(
                            verified_v2_pairs = pairs.len(),
                            verified_v3_pools = v3_pools.len(),
                            pools = snapshot.len(),
                            "canonical Ethereum WETH V2 discovery ready"
                        );
                        expanded_allowlist = expanded;
                        decimals_map.extend(decimals);
                        added_pairs = added;
                        let slippage_bps = std::env::var("SIM_SLIPPAGE_BPS")
                            .ok()
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(5);
                        net_scan = Some(NetScan {
                            gas_units,
                            tip_wei: 2_000_000_000,
                            slippage_bps,
                            min_weth,
                            pairs: pairs
                                .iter()
                                .map(|(pair, _, _)| *pair)
                                .chain(configured_pairs.iter().copied())
                                .collect(),
                        });
                        (Ok(snapshot), true)
                    } else if expanded_allowlist == allowlist {
                        (
                            load_allowlisted_reserves(&provider, &allowlist, b).await,
                            true,
                        )
                    } else {
                        refresh_with_configured_fallback(
                            b,
                            Duration::from_secs(5),
                            load_allowlisted_reserves(&provider, &expanded_allowlist, b),
                            || load_allowlisted_reserves(&provider, &allowlist, b),
                        )
                        .await
                    };
                    match refreshed {
                        Ok(fresh) => {
                            if let Some(net_scan) =
                                net_scan.as_ref().filter(|_| net_snapshot_available)
                            {
                                let priced = net_scan.report(
                                    &provider,
                                    &fresh,
                                    b,
                                    sim_min_profit,
                                    sim_max_hops,
                                );
                                if !matches!(
                                    tokio::time::timeout(std::time::Duration::from_secs(5), priced)
                                        .await,
                                    Ok(Ok(()))
                                ) {
                                    warn!(block = b, "V2 net estimates unavailable; gas or premium could not be verified");
                                }
                            }
                            let mut guard = cache.write().await;
                            if fresh.last_block() >= guard.last_block() {
                                *guard = fresh;
                            }
                            drop(guard);
                            subscribe_v2_sync(
                                provider.clone(),
                                added_pairs,
                                cache.clone(),
                                sync_notify.clone(),
                            );
                        }
                        Err(error) => warn!(%error, "allowlisted reserve refresh failed"),
                    }
                }
                Err(error) => warn!(%error, "failed to fetch block number"),
            }
        }
        let block = observed_block.max(cache.read().await.last_block());
        observed_block = block;
        let triggered_by_sync = sync_event_pending;
        sync_event_pending = false;

        let (
            pools,
            fresh_count,
            stale_count,
            cycles,
            near_misses,
            last_block,
            amounts_tried,
            v2_compatible_gross_candidates,
        ) = {
            let guard = cache.read().await;
            // Count fresh vs stale pools
            let fresh_count = guard
                .values()
                .filter(|p| block.saturating_sub(p.block) <= stale_blocks)
                .count()
                + guard
                    .v3_values()
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
                                    alloy::primitives::U256::from(f) * unit
                                        / alloy::primitives::U256::from(10u64)
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
                            for (token, reserve) in
                                [(pool.token0, pool.reserve0), (pool.token1, pool.reserve1)]
                            {
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
                    if c.gross_profit > premium && c.gross_profit - premium >= min_net {
                        kept.push(c);
                    } else if c.gross_profit > alloy::primitives::U256::ZERO {
                        near_miss_list.push((c.gross_profit, c.hops.len()));
                    }
                }
                all_cycles.extend(kept);
            }

            // Preserve both directions and start assets, selecting the best
            // probe amount only within the same ordered route.
            let deduped = best_discovery_per_ordered_route(all_cycles, premium_bps);

            // Structural ABI compatibility is NOT equivalent to executable
            // profitability; gas/eth_call and wallet controls remain unverified.
            let v2_compatible_gross_candidates = deduped
                .iter()
                .filter(|candidate| {
                    candidate.hops.iter().all(|hop| {
                        guard.get(&hop.pair).is_some_and(supports_legacy_v2_pool)
                            || guard.get_v3(&hop.pair).is_some()
                    })
                })
                .count();

            // Sort near-misses, keep top N
            near_miss_list.sort_by(|a, b| b.0.cmp(&a.0));
            near_miss_list.truncate(near_miss_count);

            (
                guard.len(),
                fresh_count,
                stale_count,
                deduped,
                near_miss_list,
                guard.last_block(),
                amounts_tried,
                v2_compatible_gross_candidates,
            )
        };

        info!(
            pools,
            fresh = fresh_count,
            stale = stale_count,
            cycles = cycles.len(),
            v2_compatible_gross_candidates,
            incompatible_gross_candidates = cycles.len() - v2_compatible_gross_candidates,
            near_misses = near_misses.len(),
            block = last_block,
            amounts_tried,
            max_hops = sim_max_hops,
            premium_bps,
            gas_cost_raw,
            "arbitrage discovery (gross candidates only; V2 compatibility counted, not live-trade signals)"
        );

        // A Sync event can reveal a market price dislocation even when the
        // fixed gross-size probes miss the profitable amount. Inspect only
        // verified, executable V2 pools with the same WETH/token topology.
        // This is a *marginal* price spread, NOT profit after two pool fees,
        // market impact, Aave, slippage or gas. A gap merely authorizes a
        // bounded, read-only SAME-BLOCK net screen, never a transaction.
        let gap_bps = if triggered_by_sync {
            let guard = cache.read().await;
            net_scan.as_ref().and_then(|scan| {
                largest_eligible_weth_v2_gap_bps(&guard, &scan.pairs, scan.min_weth)
            })
        } else {
            None
        };
        let mut cross_touched = false;
        if triggered_by_sync {
            let guard = cache.read().await;
            cross_touched = guard.values().any(|p| {
                p.block == block
                    && p.token0 != sandooo::common::constants::WETH
                    && p.token1 != sandooo::common::constants::WETH
            });
        }
        // Two canonical 30-bps V2 swaps incur ~60 bps of swap fees before
        // gas. Require an observable >75 bps marginal dislocation to avoid
        // fetching a new snapshot for normal synchronized pool noise.
        let significant_gap = cross_touched || gap_bps.is_some_and(|bps| bps >= 75);
        if triggered_by_sync
            && (v2_compatible_gross_candidates > 0 || significant_gap)
            && last_event_net_screen_block != Some(block)
        {
            if let Some(net_scan) = net_scan.as_ref() {
                last_event_net_screen_block = Some(block);
                info!(block, ?gap_bps, significant_gap,
                    v2_compatible_gross_candidates,
                    "Sync event suggests V2 price movement; rechecking pinned net economics (not a trade)");
                let fresh_head = provider.get_block_number().await;
                match fresh_head {
                    Ok(head) if head == block => {
                        let check = async {
                            let fresh =
                                load_allowlisted_reserves(&provider, &expanded_allowlist, block)
                                    .await?;
                            net_scan
                                .report(&provider, &fresh, block, sim_min_profit, sim_max_hops)
                                .await
                        };
                        if !matches!(
                            tokio::time::timeout(Duration::from_secs(5), check).await,
                            Ok(Ok(()))
                        ) {
                            warn!(
                                block,
                                "event candidate net screen inconclusive; no authorization"
                            );
                        }
                    }
                    Ok(head) => info!(
                        block,
                        head, "event candidate already superseded; skipping stale net estimate"
                    ),
                    Err(error) => warn!(block, %error,
                        "event candidate cannot confirm head; skipping net estimate"),
                }
            } else {
                info!(
                    block,
                    "event candidate pending canonical V2 net-scan configuration"
                );
            }
        }

        for cycle in cycles.iter().take(5) {
            if let (Some(live_config), Some(live_signer)) = (&live, &live_signer) {
                if let Ok(Some(header)) = provider.get_block_by_number(block.into()).await {
                    if let Some(base_fee) = header.header.base_fee_per_gas {
                        let snapshot = cache.read().await.clone();
                        match sandooo::arbitrage::live::coordinator::attempt(
                            &provider,
                            live_signer,
                            live_config,
                            &snapshot,
                            cycle,
                            block,
                            base_fee,
                        )
                        .await
                        {
                            Ok(attempt) => {
                                info!(stage = ?attempt.stage, detail = %attempt.detail, "live attempt")
                            }
                            Err(error) => warn!(%error, "live attempt failed"),
                        }
                    }
                }
            }
            // Aave premium is in loan asset raw units. This is intentionally
            // PRE-GAS only: real net profit has not been independently verified.
            let premium = cycle.amount_in * alloy::primitives::U256::from(premium_bps)
                / alloy::primitives::U256::from(10_000u64);
            let margin_before_gas = cycle.gross_profit.saturating_sub(premium);
            let contract_compatible = {
                let guard = cache.read().await;
                cycle.hops.iter().all(|hop| {
                    guard.get(&hop.pair).is_some_and(supports_legacy_v2_pool)
                        || guard.get_v3(&hop.pair).is_some()
                })
            };
            info!(
                gross_profit_raw = %cycle.gross_profit,
                margin_after_premium_before_gas_raw = %margin_before_gas,
                net_profit_verified = false,
                contract_compatible,
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
        if std::env::var("DEBUG")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            for pool in cache.read().await.values() {
                let age = block.saturating_sub(pool.block);
                tracing::debug!(
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
                sync_event_pending = true;
            }
        }
    }
}

// Configured-only reserves keep gross discovery running but cannot price the
// full set of canonical V2 routes.
async fn refresh_with_configured_fallback<T, F>(
    block: u64,
    deadline: Duration,
    expanded: impl std::future::Future<Output = anyhow::Result<T>>,
    configured: impl FnOnce() -> F,
) -> (anyhow::Result<T>, bool)
where
    F: std::future::Future<Output = anyhow::Result<T>>,
{
    match tokio::time::timeout(deadline, expanded).await {
        Ok(Ok(snapshot)) => (Ok(snapshot), true),
        _ => {
            warn!(block, "expanded V2 refresh unavailable; skipping net screen and refreshing configured pools");
            (configured().await, false)
        }
    }
}

async fn retry_v2_discovery<T, F: std::future::Future<Output = anyhow::Result<T>>>(
    pending_gas_units: &mut Option<u64>,
    deadline: Duration,
    discover: impl FnOnce() -> F,
) -> Option<(u64, T)> {
    let gas_units = (*pending_gas_units)?;
    match tokio::time::timeout(deadline, discover()).await {
        Ok(Ok(discovered)) => {
            *pending_gas_units = None;
            Some((gas_units, discovered))
        }
        _ => {
            warn!("canonical V2 discovery unavailable; using configured allowlist and retrying on refresh");
            None
        }
    }
}

fn expand_v2_allowlist(
    allowlist: &str,
    configured_pairs: &HashSet<Address>,
    pairs: &[(Address, Address, Address)],
) -> (String, Vec<Address>) {
    let mut expanded = allowlist.to_owned();
    let mut seen = configured_pairs.clone();
    let mut added = Vec::new();
    for &(pair, token0, token1) in pairs {
        if seen.insert(pair) {
            expanded.push_str(&format!(";{pair}:{token0}:{token1}:30"));
            added.push(pair);
        }
    }
    (expanded, added)
}

fn expand_v3_allowlist(
    allowlist: &str,
    seen: &[Address],
    pools: &[(Address, Address, Address, u64)],
) -> (String, Vec<Address>) {
    let mut expanded = allowlist.to_owned();
    let mut seen: HashSet<Address> = seen.iter().copied().collect();
    let mut added = Vec::new();
    for &(pool, token0, token1, fee) in pools {
        if seen.insert(pool) {
            expanded.push_str(&format!(";v3:{pool}:{token0}:{token1}:{}", fee / 100));
            added.push(pool);
        }
    }
    (expanded, added)
}
fn subscribe_v2_sync<P>(
    sync_provider: P,
    sync_pairs: Vec<Address>,
    sync_cache: Arc<RwLock<ReserveCache>>,
    event_notify: Arc<Notify>,
) where
    P: Provider<alloy::network::Ethereum> + Send + Sync + 'static,
{
    if sync_pairs.is_empty() {
        return;
    }
    tokio::spawn(async move {
        let filter = Filter::new()
            .address(sync_pairs)
            .event_signature(sync_topic());
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
            Err(error) => {
                warn!(%error, "sync log subscription unavailable; falling back to polling refresh")
            }
        }
    });
}

#[cfg(test)]
mod scanner_tests {
    use super::{best_discovery_per_ordered_route, parse_scan_enabled, pow10};
    use alloy::primitives::{address, U256};
    use sandooo::arbitrage::{Cycle, Hop};

    #[tokio::test]
    async fn configured_refresh_fallback_is_ineligible_for_net_pricing() {
        let (fallback, net_available) = super::refresh_with_configured_fallback(
            100,
            super::Duration::ZERO,
            std::future::pending(),
            || async { Ok(42) },
        )
        .await;
        assert_eq!(fallback.unwrap(), 42);
        assert!(!net_available);

        let (expanded, net_available) = super::refresh_with_configured_fallback(
            101,
            super::Duration::ZERO,
            async { Ok(43) },
            || async { panic!("successful expanded refresh must not use fallback") },
        )
        .await;
        assert_eq!(expanded.unwrap(), 43);
        assert!(net_available);
    }

    #[tokio::test]
    async fn v2_discovery_stays_pending_on_timeout_and_recovers_once() {
        let mut pending = Some(1_000_000);
        let deadline = super::Duration::ZERO;
        let timed_out = super::retry_v2_discovery(&mut pending, deadline, || {
            std::future::pending::<anyhow::Result<()>>()
        })
        .await;
        assert_eq!(timed_out, None);
        assert_eq!(pending, Some(1_000_000));

        let recovered =
            super::retry_v2_discovery(&mut pending, deadline, || async { Ok(42) }).await;
        assert_eq!(recovered, Some((1_000_000, 42)));
        assert_eq!(pending, None);

        let after_recovery = super::retry_v2_discovery(
            &mut pending,
            deadline,
            || -> std::future::Ready<anyhow::Result<()>> {
                panic!("completed discovery must not run again")
            },
        )
        .await;
        assert_eq!(after_recovery, None);
    }

    #[test]
    fn v2_allowlist_expansion_does_not_duplicate_configured_or_discovered_pairs() {
        let existing = super::Address::repeat_byte(1);
        let new = super::Address::repeat_byte(2);
        let token0 = super::Address::repeat_byte(3);
        let token1 = super::Address::repeat_byte(4);
        let configured = format!("{existing}:{token0}:{token1}");
        let (expanded, mut added) = super::expand_v2_allowlist(
            &configured,
            &[existing].into_iter().collect(),
            &[
                (existing, token0, token1),
                (new, token0, token1),
                (new, token0, token1),
            ],
        );
        assert_eq!(expanded, format!("{configured};{new}:{token0}:{token1}:30"));
        assert_eq!(added, [new]);
    }

    #[test]
    fn missing_or_invalid_scan_mode_never_activates_legacy_strategy() {
        assert!(!parse_scan_enabled(None).unwrap());
        assert!(!parse_scan_enabled(Some("false")).unwrap());
        assert!(parse_scan_enabled(Some("TRUE")).unwrap());
        assert!(parse_scan_enabled(Some("yes")).is_err());
        assert!(parse_scan_enabled(Some("")).is_err());
    }

    #[test]
    fn reverse_paths_preserved_and_best_probe_wins_per_ordered_route() {
        let a = address!("0000000000000000000000000000000000000001");
        let b = address!("0000000000000000000000000000000000000002");
        let first = address!("0000000000000000000000000000000000000010");
        let second = address!("0000000000000000000000000000000000000020");
        let make_cycle = |pairs: [alloy::primitives::Address; 2], amount: u64, gross: u64| Cycle {
            start: a,
            amount_in: U256::from(amount),
            amount_out: U256::from(amount + gross),
            gross_profit: U256::from(gross),
            hops: vec![
                Hop {
                    pair: pairs[0],
                    token_in: a,
                    token_out: b,
                    amount_in: U256::from(amount),
                    amount_out: U256::from(amount),
                },
                Hop {
                    pair: pairs[1],
                    token_in: b,
                    token_out: a,
                    amount_in: U256::from(amount),
                    amount_out: U256::from(amount + gross),
                },
            ],
            freshest_block: 100,
        };
        let selected = best_discovery_per_ordered_route(
            vec![
                make_cycle([first, second], 100, 5),
                make_cycle([second, first], 100, 3),
                make_cycle([first, second], 200, 25),
            ],
            5,
        );
        assert_eq!(selected.len(), 2, "reverse route must not disappear");
        assert_eq!(selected[0].gross_profit, U256::from(25));
        assert_eq!(selected[0].amount_in, U256::from(200));
        assert_eq!(selected[1].hops[0].pair, second);
    }

    #[test]
    fn retains_best_after_aave_margin_not_largest_gross_profits() {
        let a = address!("0000000000000000000000000000000000000001");
        let b = address!("0000000000000000000000000000000000000002");
        let p1 = address!("0000000000000000000000000000000000000010");
        let p2 = address!("0000000000000000000000000000000000000020");
        let make = |amount: u64, gross: u64| Cycle {
            start: a,
            amount_in: U256::from(amount),
            amount_out: U256::from(amount + gross),
            gross_profit: U256::from(gross),
            hops: vec![
                Hop {
                    pair: p1,
                    token_in: a,
                    token_out: b,
                    amount_in: U256::from(amount),
                    amount_out: U256::from(amount),
                },
                Hop {
                    pair: p2,
                    token_in: b,
                    token_out: a,
                    amount_in: U256::from(amount),
                    amount_out: U256::from(amount + gross),
                },
            ],
            freshest_block: 100,
        };
        // 5bps: smaller probe margin = 20-5 = 15;
        // larger probe margin = 31-25 = 6, despite greater gross profit.
        let winners = best_discovery_per_ordered_route(vec![make(50_000, 31), make(10_000, 20)], 5);
        assert_eq!(winners.len(), 1);
        assert_eq!(winners[0].amount_in, U256::from(10_000u64));
    }

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
