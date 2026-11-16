//! Structural route check: identifies whether configured V2 pools can form
//! a closed route *without* assuming the route is profitable.

use super::state::{PoolState, ReserveCache};
use alloy::primitives::Address;
use std::collections::HashSet;

/// Count directed 2..=max_hops routes that return to the initial token while
/// using each pair at most once. A positive count is NOT a profit guarantee.
pub fn count_closed_routes(cache: &ReserveCache, max_hops: usize) -> usize {
    let pools: Vec<&PoolState> = cache.values().collect();
    let mut tokens = HashSet::new();
    for p in &pools {
        tokens.insert(p.token0);
        tokens.insert(p.token1);
    }
    tokens
        .into_iter()
        .map(|start| walk(&pools, start, start, 0, max_hops, &mut HashSet::new()))
        .sum()
}

fn walk(
    pools: &[&PoolState],
    start: Address,
    current: Address,
    hops: usize,
    max_hops: usize,
    used: &mut HashSet<Address>,
) -> usize {
    if hops >= max_hops { return 0; }
    let mut count = 0usize;
    for pool in pools {
        if used.contains(&pool.pair) { continue; }
        let next = if pool.token0 == current {
            pool.token1
        } else if pool.token1 == current {
            pool.token0
        } else {
            continue;
        };
        if next == start {
            if hops + 1 >= 2 { count += 1; }
            continue;
        }
        used.insert(pool.pair);
        count += walk(pools, start, next, hops + 1, max_hops, used);
        used.remove(&pool.pair);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, U256};
    fn pool(pair: Address, token0: Address, token1: Address) -> PoolState {
        PoolState {
            pair, token0, token1,
            reserve0: U256::from(1_000u64),
            reserve1: U256::from(1_000u64),
            fee_bps: 30, block: 1,
        }
    }
    #[test]
    fn no_closed_path_with_single_or_disjoint_pools() {
        let a = address!("0000000000000000000000000000000000000001");
        let b = address!("0000000000000000000000000000000000000002");
        let c = address!("0000000000000000000000000000000000000003");
        let d = address!("0000000000000000000000000000000000000004");
        let mut cache = ReserveCache::default();
        cache.insert(pool(address!("0000000000000000000000000000000000000010"),a,b));
        assert_eq!(count_closed_routes(&cache, 3), 0);
        cache.insert(pool(address!("0000000000000000000000000000000000000020"),c,d));
        assert_eq!(count_closed_routes(&cache, 3), 0);
    }
    #[test]
    fn detects_parallel_pair_and_triangle_routes() {
        let a = address!("0000000000000000000000000000000000000001");
        let b = address!("0000000000000000000000000000000000000002");
        let c = address!("0000000000000000000000000000000000000003");
        let mut cache = ReserveCache::default();
        cache.insert(pool(address!("0000000000000000000000000000000000000010"),a,b));
        cache.insert(pool(address!("0000000000000000000000000000000000000020"),a,b));
        assert!(count_closed_routes(&cache,2)>0);
        let mut triangle = ReserveCache::default();
        triangle.insert(pool(address!("0000000000000000000000000000000000000010"),a,b));
        triangle.insert(pool(address!("0000000000000000000000000000000000000020"),b,c));
        triangle.insert(pool(address!("0000000000000000000000000000000000000030"),c,a));
        assert_eq!(count_closed_routes(&triangle,2),0);
        assert!(count_closed_routes(&triangle,3)>0);
    }
}
