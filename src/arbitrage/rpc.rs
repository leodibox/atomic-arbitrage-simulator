use super::state::{PoolState, ReserveCache};
use alloy::{
    network::Ethereum,
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};

sol! {
    #[sol(rpc)]
    interface V2Pair {
        function token0() external view returns (address);
        function token1() external view returns (address);
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }

    #[sol(rpc)]
    interface IERC20Meta {
        function decimals() external view returns (uint8);
    }
}

/// Loads only explicitly configured pools. Format:
/// `pair:token0:token1;pair:token0:token1`
/// Reserves are fetched with ordinary eth_call, not debug tracing.
pub async fn load_allowlisted_reserves<P>(
    provider: &P,
    allowlist: &str,
    block: u64,
) -> Result<ReserveCache>
where
    P: Provider<Ethereum> + Clone,
{
    let mut cache = ReserveCache::default();
    for item in allowlist.split(';').map(str::trim).filter(|v| !v.is_empty()) {
        let fields: Vec<_> = item.split(':').map(str::trim).collect();
        if fields.len() != 3 {
            anyhow::bail!("POOL_ALLOWLIST entry must be pair:token0:token1");
        }
        let pair: Address = fields[0].parse().context("invalid pair address")?;
        let token0: Address = fields[1].parse().context("invalid token0 address")?;
        let token1: Address = fields[2].parse().context("invalid token1 address")?;

        let contract = V2Pair::new(pair, provider.clone());
        let reserves = contract.getReserves().call().await?;
        cache.insert(PoolState {
            pair,
            token0,
            token1,
            reserve0: {
                let limbs = reserves.reserve0.as_limbs();
                U256::from_limbs([limbs[0], limbs[1], 0, 0])
            },
            reserve1: {
                let limbs = reserves.reserve1.as_limbs();
                U256::from_limbs([limbs[0], limbs[1], 0, 0])
            },
            fee_bps: 30,
            block,
        });
    }
    Ok(cache)
}

/// Loads ERC20 decimals for a set of token addresses (best-effort, defaults to 18).
pub async fn load_token_decimals<P>(
    provider: &P,
    tokens: impl IntoIterator<Item = Address>,
) -> HashMap<Address, u8>
where
    P: Provider<Ethereum> + Clone,
{
    let unique: HashSet<Address> = tokens.into_iter().collect();
    let mut out = HashMap::new();
    for token in unique {
        let contract = IERC20Meta::new(token, provider.clone());
        let decimals = match contract.decimals().call().await {
            Ok(d) => d,
            Err(_) => 18,
        };
        out.insert(token, decimals);
    }
    out
}

/// Validate the pool/token triples once at startup against the pool contract.
/// A typo in POOL_ALLOWLIST otherwise creates misleading or impossible routes.
/// Unlike reserve reconciliation, this metadata verification is not polled.
pub async fn verify_allowlisted_tokens<P>(provider: &P, cache: &ReserveCache) -> Result<()>
where
    P: Provider<Ethereum> + Clone,
{
    for pool in cache.values() {
        let contract = V2Pair::new(pool.pair, provider.clone());
        let actual0 = contract.token0().call().await
            .with_context(|| format!("token0() failed for pair {}", pool.pair))?;
        let actual1 = contract.token1().call().await
            .with_context(|| format!("token1() failed for pair {}", pool.pair))?;
        anyhow::ensure!(actual0 == pool.token0 && actual1 == pool.token1,
            "POOL_ALLOWLIST token mismatch for pair {}: configured ({}, {}), on-chain ({}, {})",
            pool.pair, pool.token0, pool.token1, actual0, actual1);
        anyhow::ensure!(actual0 != actual1,
            "pair {} reports identical token0 and token1", pool.pair);
    }
    Ok(())
}
