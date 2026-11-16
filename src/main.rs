use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::Filter;
use alloy::transports::ws::WsConnect;
use sandooo::arbitrage::{
    find_cycles,
    rpc::{load_allowlisted_reserves, load_token_decimals},
    state::sync_topic,
};
use sandooo::common::{constants::get_env, pools::load_all_pools, streams::*};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::task::JoinSet;
use tracing::{info, warn};
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

    let sim_amount_override = std::env::var("SIM_AMOUNT_RAW")
        .ok()
        .and_then(|v| v.parse::<u64>().ok());
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
        .unwrap_or(15);
    let premium_bps = std::env::var("SIM_AAVE_PREMIUM_BPS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5); // Aave V3 flashLoanSimple typical premium
    let gas_cost_raw = std::env::var("SIM_GAS_COST_RAW")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let block = provider.get_block_number().await?;
    let initial = load_allowlisted_reserves(&provider, &allowlist, block).await?;
    let pair_addresses: Vec<_> = initial.pairs().copied().collect();
    let unique_tokens: Vec<_> = initial
        .values()
        .flat_map(|p| [p.token0, p.token1])
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let decimals_map = load_token_decimals(&provider, unique_tokens).await;
    let cache = Arc::new(RwLock::new(initial));

    // Event-driven reserve updates from Sync logs (best-effort).
    let sync_cache = cache.clone();
    let sync_provider = provider.clone();
    let sync_pairs = pair_addresses.clone();
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
                    guard.apply_sync_log(pair, data, block);
                }
                warn!("sync log stream ended");
            }
            Err(error) => warn!(%error, "sync log subscription unavailable; falling back to polling refresh"),
        }
    });

    loop {
        // Periodic full refresh as fallback / reconciliation.
        let block = match provider.get_block_number().await {
            Ok(b) => b,
            Err(error) => {
                warn!(%error, "failed to fetch block number");
                tokio::time::sleep(std::time::Duration::from_secs(sim_interval)).await;
                continue;
            }
        };
        match load_allowlisted_reserves(&provider, &allowlist, block).await {
            Ok(fresh) => {
                let mut guard = cache.write().await;
                *guard = fresh;
            }
            Err(error) => warn!(%error, "allowlisted reserve refresh failed"),
        }

        let (pools, mut cycles, last_block, amounts_tried) = {
            let guard = cache.read().await;
            let mut by_decimals: HashMap<u8, Vec<alloy::primitives::Address>> = HashMap::new();
            let mut seen: HashSet<alloy::primitives::Address> = HashSet::new();
            for pool in guard.values() {
                for token in [pool.token0, pool.token1] {
                    if seen.insert(token) {
                        let d = *decimals_map.get(&token).unwrap_or(&18u8);
                        by_decimals.entry(d).or_default().push(token);
                    }
                }
            }

            let min_net = alloy::primitives::U256::from(sim_min_profit);
            let mut all_cycles = Vec::new();
            let mut amounts_tried = 0usize;

            let run_amounts: Vec<(alloy::primitives::U256, Vec<alloy::primitives::Address>)> =
                if let Some(amount) = sim_amount_override {
                    let starts: Vec<_> = seen.iter().copied().collect();
                    vec![(alloy::primitives::U256::from(amount), starts)]
                } else {
                    let mut runs = Vec::new();
                    for (decimals, starts) in &by_decimals {
                        let unit = alloy::primitives::U256::from(10u64).pow(alloy::primitives::U256::from(*decimals));
                        // Try a few notionals scaled to token decimals to improve discovery.
                        let factors: &[u64] = if *decimals <= 8 {
                            &[1_000, 10_000, 50_000]
                        } else if *decimals <= 9 {
                            &[100, 1_000, 10_000]
                        } else {
                            // 18-dec (e.g. WETH/DAI): 0.1, 1, 10, 50 units
                            // 6-dec (e.g. USDC/USDT): 100, 1_000, 10_000, 50_000 units
                            if *decimals == 6 {
                                &[100, 1_000, 10_000, 50_000]
                            } else {
                                // represent 0.1 as factor 1 with unit/10 handled below
                                &[1, 10, 50]
                            }
                        };
                        for &f in factors {
                            let amount = if *decimals >= 18 {
                                // f in {1,10,50} => 0.1,1,5 units? better: 1,10,50 units and include 0.5 via /2
                                if f == 1 {
                                    unit / alloy::primitives::U256::from(10u64) // 0.1
                                } else if f == 10 {
                                    unit // 1
                                } else {
                                    alloy::primitives::U256::from(10u64) * unit // 10
                                }
                            } else if *decimals == 6 {
                                alloy::primitives::U256::from(f) * unit
                            } else {
                                alloy::primitives::U256::from(f) * unit
                            };
                            if amount > alloy::primitives::U256::ZERO {
                                runs.push((amount, starts.clone()));
                            }
                        }
                        // add larger size for 18-dec
                        if *decimals >= 18 {
                            runs.push((alloy::primitives::U256::from(50u64) * unit, starts.clone()));
                        }
                    }
                    runs
                };

            for (amount, starts) in run_amounts {
                amounts_tried += 1;
                let mut found = find_cycles(
                    &guard,
                    &starts,
                    amount,
                    sim_max_hops,
                    alloy::primitives::U256::ZERO,
                );
                let premium = amount * alloy::primitives::U256::from(premium_bps)
                    / alloy::primitives::U256::from(10_000u64);
                let costs = premium + alloy::primitives::U256::from(gas_cost_raw);
                found.retain(|c| c.gross_profit > costs && c.gross_profit - costs >= min_net);
                all_cycles.extend(found);
            }

            // de-dup by path signature + amount range roughly: keep best net per (pairs,hops)
            all_cycles.sort_by(|a, b| b.gross_profit.cmp(&a.gross_profit));
            (guard.len(), all_cycles, guard.last_block(), amounts_tried)
        };

        info!(
            pools,
            cycles = cycles.len(),
            block = last_block,
            amounts_tried,
            max_hops = sim_max_hops,
            premium_bps,
            gas_cost_raw,
            "arbitrage simulation"
        );
        for cycle in cycles.iter().take(5) {
            let premium = cycle.amount_in * alloy::primitives::U256::from(premium_bps)
                / alloy::primitives::U256::from(10_000u64);
            let net = cycle
                .gross_profit
                .saturating_sub(premium + alloy::primitives::U256::from(gas_cost_raw));
            info!(
                gross_profit = %cycle.gross_profit,
                net_profit = %net,
                amount_in = %cycle.amount_in,
                amount_out = %cycle.amount_out,
                hops = cycle.hops.len(),
                freshest_block = cycle.freshest_block,
                "simulated cycle"
            );
        }

        tokio::time::sleep(std::time::Duration::from_secs(sim_interval)).await;
    }
}
