use super::state::{PoolState, ReserveCache, V3PoolState};
use alloy::primitives::{Address, U256};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Hop {
    pub pair: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub amount_out: U256,
}

#[derive(Debug, Clone)]
pub struct Cycle {
    pub start: Address,
    pub amount_in: U256,
    pub amount_out: U256,
    pub gross_profit: U256,
    pub hops: Vec<Hop>,
    pub freshest_block: u64,
}

/// Unified pool trait for quoting
enum PoolRef<'a> {
    V2(&'a PoolState),
    V3(&'a V3PoolState),
}

impl<'a> PoolRef<'a> {
    fn pair(&self) -> Address {
        match self {
            PoolRef::V2(p) => p.pair,
            PoolRef::V3(p) => p.pair,
        }
    }
    fn quote(&self, input: Address, amount: U256) -> Option<(Address, U256)> {
        match self {
            PoolRef::V2(p) => p.quote(input, amount),
            PoolRef::V3(p) => p.quote(input, amount),
        }
    }
    fn block(&self) -> u64 {
        match self {
            PoolRef::V2(p) => p.block,
            PoolRef::V3(p) => p.block,
        }
    }
}

/// Finds closed 2-4 hop cycles from the current reserve cache.
/// It only quotes locally; it never signs or submits a transaction.
pub fn find_cycles(
    cache: &ReserveCache,
    starts: &[Address],
    amount_in: U256,
    max_hops: usize,
    min_profit: U256,
) -> Vec<Cycle> {
    let mut results = Vec::new();
    // Collect all pools (V2 + V3)
    let pools: Vec<PoolRef> = cache
        .values()
        .map(PoolRef::V2)
        .chain(cache.v3_values().map(PoolRef::V3))
        .collect();

    for &start in starts {
        walk(
            &pools,
            start,
            start,
            amount_in,
            max_hops,
            min_profit,
            &mut HashSet::new(),
            &mut Vec::new(),
            &mut results,
            0,
        );
    }
    results
}

fn walk(
    pools: &[PoolRef],
    start: Address,
    current: Address,
    amount: U256,
    max_hops: usize,
    min_profit: U256,
    used_pairs: &mut HashSet<Address>,
    hops: &mut Vec<Hop>,
    results: &mut Vec<Cycle>,
    freshest_block: u64,
) {
    if hops.len() >= max_hops { return; }

    for pool in pools {
        if used_pairs.contains(&pool.pair()) { continue; }
        let Some((next, output)) = pool.quote(current, amount) else { continue; };
        used_pairs.insert(pool.pair());
        hops.push(Hop {
            pair: pool.pair(),
            token_in: current,
            token_out: next,
            amount_in: amount,
            amount_out: output,
        });

        let initial_amount = hops.first().map(|hop| hop.amount_in).unwrap_or(amount);
        if next == start && hops.len() >= 2 && output > initial_amount + min_profit {
            results.push(Cycle {
                start,
                amount_in: initial_amount,
                amount_out: output,
                gross_profit: output - initial_amount,
                hops: hops.clone(),
                freshest_block: freshest_block.max(pool.block()),
            });
        } else if next != start {
            walk(
                pools,
                start,
                next,
                output,
                max_hops,
                min_profit,
                used_pairs,
                hops,
                results,
                freshest_block.max(pool.block()),
            );
        }

        hops.pop();
        used_pairs.remove(&pool.pair());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    #[test]
    fn finds_profitable_two_pool_cycle() {
        let a = address!("0000000000000000000000000000000000000001");
        let b = address!("0000000000000000000000000000000000000002");
        let c = address!("0000000000000000000000000000000000000003");
        let mut cache = ReserveCache::default();
        cache.insert(PoolState {
            pair: address!("0000000000000000000000000000000000000010"),
            token0: a, token1: b,
            reserve0: U256::from(1_000_000u64), reserve1: U256::from(3_000_000u64),
            fee_bps: 30, block: 10,
        });
        cache.insert(PoolState {
            pair: address!("0000000000000000000000000000000000000020"),
            token0: b, token1: a,
            reserve0: U256::from(1_000_000u64), reserve1: U256::from(900_000u64),
            fee_bps: 30, block: 10,
        });
        cache.insert(PoolState {
            pair: address!("0000000000000000000000000000000000000030"),
            token0: a, token1: c,
            reserve0: U256::from(1u64), reserve1: U256::from(1u64),
            fee_bps: 30, block: 10,
        });
        let cycles = find_cycles(&cache, &[a], U256::from(1_000u64), 3, U256::from(1u64));
        assert!(!cycles.is_empty());
        assert!(cycles[0].gross_profit > U256::ZERO);
    }
}