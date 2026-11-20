use super::state::{PoolState, ReserveCache, V3PoolState};
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
        function factory() external view returns (address);
        function token0() external view returns (address);
        function token1() external view returns (address);
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }

    #[sol(rpc)]
    interface V3Pool {
        function token0() external view returns (address);
        function token1() external view returns (address);
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
        function liquidity() external view returns (uint128);
        function fee() external view returns (uint24);
    }

    #[sol(rpc)]
    interface IERC20Meta {
        function decimals() external view returns (uint8);
    }
}

/// Loads only explicitly configured pools. Format:
/// `pair:token0:token1` or `pair:token0:token1:fee_bps`
/// Reserves are fetched concurrently with ordinary eth_call, not debug tracing.
pub async fn load_allowlisted_reserves<P>(
    provider: &P,
    allowlist: &str,
    block: u64,
) -> Result<ReserveCache>
where
    P: Provider<Ethereum> + Clone + Send + Sync + 'static,
{
    let entries: Vec<_> = allowlist
        .split(';')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect();

    // Separate V2 and V3 entries
    let mut v2_entries = Vec::new();
    let mut v3_entries = Vec::new();
    for item in &entries {
        if item.starts_with("v3:") {
            v3_entries.push(item.trim_start_matches("v3:"));
        } else {
            v2_entries.push(*item);
        }
    }

    let mut cache = ReserveCache::default();

    // Fetch V2 reserves concurrently
    if !v2_entries.is_empty() {
        let mut parsed = Vec::with_capacity(v2_entries.len());
        for item in &v2_entries {
            let fields: Vec<_> = item.split(':').map(str::trim).collect();
            if fields.len() != 3 && fields.len() != 4 {
                anyhow::bail!("V2 entry must be pair:token0:token1 or pair:token0:token1:fee_bps");
            }
            let pair: Address = fields[0].parse().context("invalid pair address")?;
            let token0: Address = fields[1].parse().context("invalid token0 address")?;
            let token1: Address = fields[2].parse().context("invalid token1 address")?;
            let fee_bps: u64 = if fields.len() == 4 {
                fields[3].parse().context("invalid fee_bps")?
            } else {
                30
            };
            parsed.push((pair, token0, token1, fee_bps));
        }

        let mut handles = Vec::with_capacity(parsed.len());
        for (pair, token0, token1, fee_bps) in parsed {
            let provider = provider.clone();
            handles.push(tokio::spawn(async move {
                let contract = V2Pair::new(pair, provider);
                let reserves = contract.getReserves().block(block.into()).call().await?;
                Ok::<_, anyhow::Error>((pair, token0, token1, fee_bps, reserves))
            }));
        }

        for handle in handles {
            let (pair, token0, token1, fee_bps, reserves) = handle.await??;
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
                fee_bps,
                block,
            });
        }
    }

    // Fetch V3 pool state concurrently
    if !v3_entries.is_empty() {
        let mut parsed = Vec::with_capacity(v3_entries.len());
        for item in &v3_entries {
            let fields: Vec<_> = item.split(':').map(str::trim).collect();
            if fields.len() != 3 && fields.len() != 4 {
                anyhow::bail!("V3 entry must be pair:token0:token1 or pair:token0:token1:fee_bps");
            }
            let pair: Address = fields[0].parse().context("invalid pair address")?;
            let token0: Address = fields[1].parse().context("invalid token0 address")?;
            let token1: Address = fields[2].parse().context("invalid token1 address")?;
            let fee_bps: u64 = if fields.len() == 4 {
                fields[3].parse().context("invalid fee_bps")?
            } else {
                30
            };
            parsed.push((pair, token0, token1, fee_bps));
        }

        let mut handles = Vec::with_capacity(parsed.len());
        for (pair, token0, token1, fee_bps) in parsed {
            let provider = provider.clone();
            handles.push(tokio::spawn(async move {
                let contract = V3Pool::new(pair, provider);
                let slot0 = contract.slot0().block(block.into()).call().await?;
                let liq = contract.liquidity().block(block.into()).call().await?;
                let fee = contract.fee().block(block.into()).call().await?;
                Ok::<_, anyhow::Error>((pair, token0, token1, fee_bps, slot0, liq, fee))
            }));
        }

        for handle in handles {
            let (pair, token0, token1, fee_bps, slot0, liq, fee) = handle.await??;
            let sqrt_price = U256::from(slot0.sqrtPriceX96);
            let liquidity = U256::from(liq);
            let tick: i128 = i128::try_from(slot0.tick).unwrap_or(0);
            // Use on-chain fee if available, else config
            let actual_fee_bps = if fee > alloy::primitives::Uint::<24, 1>::ZERO { fee.to::<u64>() / 100 } else { fee_bps };
            cache.insert_v3(V3PoolState {
                pair,
                token0,
                token1,
                sqrt_price_x96: sqrt_price,
                liquidity,
                tick,
                fee_bps: actual_fee_bps,
                block,
            });
        }
    }

    Ok(cache)
}

/// Verify the configured token ordering against on-chain metadata before scanning.
/// Invalid POOL_ALLOWLIST entries can otherwise fabricate profitable routes.
pub async fn verify_allowlisted_tokens<P>(provider: &P, cache: &ReserveCache) -> Result<()>
where
    P: Provider<Ethereum> + Clone,
{
    for pool in cache.values() {
        anyhow::ensure!(pool.fee_bps < 10_000, "invalid V2 fee on pair {}", pool.pair);
        let contract = V2Pair::new(pool.pair, provider.clone());
        let actual0 = contract.token0().call().await
            .with_context(|| format!("token0() failed for V2 pair {}", pool.pair))?;
        let actual1 = contract.token1().call().await
            .with_context(|| format!("token1() failed for V2 pair {}", pool.pair))?;
        anyhow::ensure!(actual0 == pool.token0 && actual1 == pool.token1
            && actual0 != actual1,
            "POOL_ALLOWLIST token mismatch for V2 pair {}", pool.pair);
    }
    for pool in cache.v3_values() {
        anyhow::ensure!(pool.fee_bps < 10_000, "invalid V3 fee on pair {}", pool.pair);
        let contract = V3Pool::new(pool.pair, provider.clone());
        let actual0 = contract.token0().call().await
            .with_context(|| format!("token0() failed for V3 pool {}", pool.pair))?;
        let actual1 = contract.token1().call().await
            .with_context(|| format!("token1() failed for V3 pool {}", pool.pair))?;
        anyhow::ensure!(actual0 == pool.token0 && actual1 == pool.token1
            && actual0 != actual1,
            "POOL_ALLOWLIST token mismatch for V3 pool {}", pool.pair);
    }
    Ok(())
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

sol! {
    #[sol(rpc)]
    interface CanonicalV2Factory {
        function getPair(address tokenA, address tokenB) external view returns (address);
    }
}

/// Resolve only known Ethereum V2 factories and standard WETH markets.
pub async fn discover_weth_v2<P: Provider<Ethereum> + Clone>(
    provider: &P,
    block: u64,
    min_weth: U256,
) -> Result<Vec<(Address, Address, Address)>> {
    use crate::common::constants::{FACTORY_V2, WETH};
    use alloy::primitives::address;

    anyhow::ensure!(
        provider.get_chain_id().await? == 1,
        "V2 discovery requires Ethereum mainnet"
    );
    let factories = [
        FACTORY_V2,
        address!("C0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"),
    ];
    let markets = [
        address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        address!("dAC17F958D2ee523a2206206994597C13D831ec7"),
        address!("6B175474E89094C44Da98b954EedeAC495271d0F"),
    ];
    let mut pools = Vec::new();
    for factory in factories {
        let registry = CanonicalV2Factory::new(factory, provider.clone());
        for token in markets {
            let pair = registry
                .getPair(WETH, token)
                .block(block.into())
                .call()
                .await?;
            if pair.is_zero() {
                continue;
            }
            let contract = V2Pair::new(pair, provider.clone());
            let token0 = contract.token0().block(block.into()).call().await?;
            let token1 = contract.token1().block(block.into()).call().await?;
            let actual_factory = contract.factory().block(block.into()).call().await?;
            anyhow::ensure!(
                actual_factory == factory
                    && ((token0 == WETH && token1 == token) || (token1 == WETH && token0 == token)),
                "factory or token mismatch for discovered V2 pair {pair}"
            );
            let reserves = contract.getReserves().block(block.into()).call().await?;
            let weth_reserve = if token0 == WETH {
                reserves.reserve0
            } else {
                reserves.reserve1
            };
            if !reserves.reserve0.is_zero()
                && !reserves.reserve1.is_zero()
                && U256::from(weth_reserve) >= min_weth
            {
                pools.push((pair, token0, token1));
            }
        }
    }
    Ok(pools)
}
