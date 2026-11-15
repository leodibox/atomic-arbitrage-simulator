use alloy::providers::{Provider, ProviderBuilder};
use alloy::transports::ws::WsConnect;
use sandooo::arbitrage::{find_cycles, rpc::load_allowlisted_reserves};
use sandooo::common::{constants::get_env, pools::load_all_pools, streams::*};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
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
    P: Provider<alloy::network::Ethereum> + Clone + 'static,
{
    let allowlist = std::env::var("POOL_ALLOWLIST").unwrap_or_default();
    if allowlist.trim().is_empty() {
        warn!("POOL_ALLOWLIST is empty; worker will remain idle");
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    }

    loop {
        let block = provider.get_block_number().await?;
        match load_allowlisted_reserves(&provider, &allowlist, block).await {
            Ok(cache) => {
                let starts: Vec<_> = cache
                    .values()
                    .flat_map(|p| [p.token0, p.token1])
                    .collect();
                let cycles = find_cycles(
                    &cache,
                    &starts,
                    alloy::primitives::U256::from(1_000_000u64),
                    3,
                    alloy::primitives::U256::from(1u64),
                );
                info!(pools = cache.len(), cycles = cycles.len(), block, "arbitrage simulation");
                for cycle in cycles.iter().take(5) {
                    info!(
                        gross_profit = %cycle.gross_profit,
                        hops = cycle.hops.len(),
                        freshest_block = cycle.freshest_block,
                        "simulated cycle"
                    );
                }
            }
            Err(error) => warn!(%error, "allowlisted reserve refresh failed"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
    }
}
