use super::state::{PoolState, ReserveCache};
use alloy::{
    network::Ethereum,
    primitives::{Address, U256},
    providers::Provider,
    sol,
};
use anyhow::{Context, Result};

sol! {
    #[sol(rpc)]
    interface V2Pair {
        function token0() external view returns (address);
        function token1() external view returns (address);
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
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
