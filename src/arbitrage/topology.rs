//! Validate whether allowlisted V2 and V3 pools contain 2–3 hop closed
//! token routes. These are structural routes, NOT profitable opportunities.
use super::state::ReserveCache;
use alloy::primitives::Address;
use std::collections::HashSet;

pub fn count_closed_routes(cache: &ReserveCache, max_hops: usize) -> usize {
    let edges: Vec<(Address, Address, Address)> = cache
        .values()
        .map(|p| (p.pair, p.token0, p.token1))
        .chain(cache.v3_values().map(|p| (p.pair, p.token0, p.token1)))
        .collect();
    let mut starts = HashSet::new();
    for &(_, a, b) in &edges {
        starts.insert(a);
        starts.insert(b);
    }
    starts
        .into_iter()
        .map(|start| walk(&edges, start, start, 0, max_hops, &mut HashSet::new()))
        .sum()
}

fn walk(
    edges: &[(Address, Address, Address)],
    start: Address,
    current: Address,
    hops: usize,
    max_hops: usize,
    used: &mut HashSet<Address>,
) -> usize {
    if hops >= max_hops {
        return 0;
    }
    let mut found = 0;
    for &(pair, a, b) in edges {
        if used.contains(&pair) {
            continue;
        }
        let next = if a == current {
            b
        } else if b == current {
            a
        } else {
            continue;
        };
        if next == start {
            if hops + 1 >= 2 {
                found += 1;
            }
        } else {
            used.insert(pair);
            found += walk(edges, start, next, hops + 1, max_hops, used);
            used.remove(&pair);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arbitrage::state::{PoolState, V3PoolState};
    use alloy::primitives::{address, U256};

    #[test]
    fn disjoint_pools_cannot_form_cycles() {
        let mut c = ReserveCache::default();
        c.insert(PoolState {
            pair: address!("0000000000000000000000000000000000000010"),
            token0: address!("0000000000000000000000000000000000000001"),
            token1: address!("0000000000000000000000000000000000000002"),
            reserve0: U256::from(100u64),
            reserve1: U256::from(100u64),
            fee_bps: 30,
            block: 1,
        });
        assert_eq!(count_closed_routes(&c, 3), 0);
    }

    #[test]
    fn can_form_two_hop_v2_v3_closed_route() {
        let a = address!("0000000000000000000000000000000000000001");
        let b = address!("0000000000000000000000000000000000000002");
        let mut c = ReserveCache::default();
        c.insert(PoolState {
            pair: address!("0000000000000000000000000000000000000010"),
            token0: a,
            token1: b,
            reserve0: U256::from(100u64),
            reserve1: U256::from(100u64),
            fee_bps: 30,
            block: 1,
        });
        c.insert_v3(V3PoolState {
            pair: address!("0000000000000000000000000000000000000020"),
            token0: a,
            token1: b,
            sqrt_price_x96: U256::from(1u128) << 96,
            liquidity: U256::from(100_000u64),
            tick: 0,
            fee_bps: 5,
            block: 1,
        });
        assert_eq!(count_closed_routes(&c, 1), 0);
        assert!(count_closed_routes(&c, 2) > 0);
    }
}
