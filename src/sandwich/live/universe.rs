use super::config::SandwichConfig;
use crate::arbitrage::rpc::{
    discover_weth_v2, load_allowlisted_reserves, verify_allowlisted_tokens,
};
use crate::arbitrage::state::{sync_topic, PoolState, ReserveCache};
use crate::common::constants::WETH;
use alloy::network::Ethereum;
use alloy::primitives::Address;
use alloy::sol;

sol! {
    #[sol(rpc)]
    interface IV2Meta {
        function token0() external view returns (address);
        function token1() external view returns (address);
    }
}
use alloy::providers::Provider;
use alloy::rpc::types::Filter;
use anyhow::Result;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct PairMeta {
    pub token0: Address,
    pub token1: Address,
}

pub struct Universe {
    cache: Arc<RwLock<ReserveCache>>,
    pairs: Arc<RwLock<HashMap<Address, PairMeta>>>,
    index: Arc<RwLock<HashMap<(Address, Address), Address>>>,
    head: Arc<AtomicU64>,
}

fn key(a: Address, b: Address) -> (Address, Address) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

impl Universe {
    pub fn head(&self) -> u64 {
        self.head.load(Ordering::Relaxed)
    }

    pub fn set_head(&self, block: u64) {
        self.head.fetch_max(block, Ordering::Relaxed);
    }

    pub fn pair_for(&self, a: Address, b: Address) -> Option<(Address, bool)> {
        let map = self.index.try_read().ok()?;
        let pair = *map.get(&key(a, b))?;
        let meta = self.pairs.try_read().ok()?.get(&pair)?.clone();
        Some((pair, meta.token0 == WETH))
    }

    pub fn is_known(&self, pair: &Address) -> bool {
        self.pairs
            .try_read()
            .map(|m| m.contains_key(pair))
            .unwrap_or(false)
    }

    pub async fn fresh_pool(&self, pair: &Address, max_age: u64) -> Option<PoolState> {
        let guard = self.cache.read().await;
        let pool = guard.get(pair)?.clone();
        if self.head().saturating_sub(pool.block) > max_age {
            return None;
        }
        Some(pool)
    }

    async fn absorb(&self, pools: Vec<(Address, Address, Address)>) {
        let mut pairs = self.pairs.write().await;
        let mut index = self.index.write().await;
        for (pair, t0, t1) in pools {
            pairs.insert(
                pair,
                PairMeta {
                    token0: t0,
                    token1: t1,
                },
            );
            index.insert(key(t0, t1), pair);
        }
    }

    async fn reload(
        &self,
        provider: &(impl Provider<Ethereum> + Clone + 'static),
        cfg: &SandwichConfig,
    ) -> Result<usize> {
        let block = provider.get_block_number().await?;
        self.set_head(block);
        let mut discovered = discover_weth_v2(provider, block, cfg.min_pool_weth).await?;
        // Configured pools bypass the reserve floor: they are listed on purpose.
        let extra: Vec<Address> = std::env::var("SANDWICH_EXTRA_POOLS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|v| v.trim().parse::<Address>().ok())
            .collect();
        for pair in &extra {
            if discovered.iter().any(|(p, _, _)| p == pair) {
                continue;
            }
            let meta = IV2Meta::new(*pair, provider.clone());
            let (Ok(t0), Ok(t1)) = (meta.token0().call().await, meta.token1().call().await) else {
                warn!(%pair, "extra pool unreadable; skipped");
                continue;
            };
            if t0 != WETH && t1 != WETH {
                warn!(%pair, "extra pool is not WETH paired; skipped");
                continue;
            }
            discovered.push((*pair, t0, t1));
        }
        if discovered.is_empty() {
            return Ok(0);
        }

        let mut allow = String::new();
        for (pair, t0, t1) in &discovered {
            allow.push_str(&format!("{pair}:{t0}:{t1}:30;"));
        }
        self.absorb(discovered).await;

        let cache = load_allowlisted_reserves(provider, &allow, block).await?;
        verify_allowlisted_tokens(provider, &cache).await?;

        let mut guard = self.cache.write().await;
        for pool in cache.values() {
            guard.insert(pool.clone());
        }
        Ok(guard.len())
    }

    pub async fn subscribe_sync<P>(&self, provider: P)
    where
        P: Provider<Ethereum> + Send + Sync + 'static,
    {
        let cache = self.cache.clone();
        let head = self.head.clone();
        tokio::spawn(async move {
            loop {
                let filter = Filter::new().event_signature(sync_topic());
                match provider.subscribe_logs(&filter).await {
                    Ok(sub) => {
                        let mut stream = sub.into_stream();
                        while let Some(log) = stream.next().await {
                            let pair = log.address();
                            let block = log.block_number.unwrap_or_default();
                            head.fetch_max(block, Ordering::Relaxed);
                            cache.write().await.apply_sync_log(
                                pair,
                                log.data().data.as_ref(),
                                block,
                            );
                        }
                        warn!("sync stream ended, resubscribing");
                    }
                    Err(e) => {
                        warn!(%e, "sync subscription unavailable, retrying");
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    }
                }
            }
        });
    }
}

pub async fn build(
    provider: &(impl Provider<Ethereum> + Clone + 'static),
    cfg: Arc<SandwichConfig>,
) -> Result<Arc<Universe>> {
    let head = provider.get_block_number().await?;
    let universe = Arc::new(Universe {
        cache: Arc::new(RwLock::new(ReserveCache::default())),
        pairs: Arc::new(RwLock::new(HashMap::new())),
        index: Arc::new(RwLock::new(HashMap::new())),
        head: Arc::new(AtomicU64::new(head)),
    });
    let n = universe.reload(provider, &cfg).await?;
    info!(pools = n, block = head, "sandwich universe ready");
    Ok(universe)
}

pub fn spawn_refresh<P>(provider: P, cfg: Arc<SandwichConfig>, universe: Arc<Universe>)
where
    P: Provider<Ethereum> + Clone + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let mut last = universe.head();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let head = provider.get_block_number().await.unwrap_or(last);
            universe.set_head(head);
            if head.saturating_sub(last) >= cfg.refresh_blocks {
                last = head;
                match universe.reload(&provider, &cfg).await {
                    Ok(n) => info!(pools = n, "universe refreshed"),
                    Err(e) => warn!(%e, "universe refresh failed"),
                }
            }
        }
    });
}
