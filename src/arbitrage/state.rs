use alloy::primitives::{Address, B256, U256};
use std::collections::HashMap;

/// Uniswap V2-compatible pool state used by the local quote engine.
#[derive(Debug, Clone)]
pub struct PoolState {
    pub pair: Address,
    pub token0: Address,
    pub token1: Address,
    pub reserve0: U256,
    pub reserve1: U256,
    pub fee_bps: u64,
    pub block: u64,
}

impl PoolState {
    pub fn quote(&self, input: Address, amount: U256) -> Option<(Address, U256)> {
        let (reserve_in, reserve_out, output) = if input == self.token0 {
            (self.reserve0, self.reserve1, self.token1)
        } else if input == self.token1 {
            (self.reserve1, self.reserve0, self.token0)
        } else {
            return None;
        };
        if amount.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
            return None;
        }

        let fee_denominator = U256::from(10_000u64);
        let fee_multiplier = fee_denominator - U256::from(self.fee_bps);
        let amount_after_fee = amount * fee_multiplier;
        let output_amount = (amount_after_fee * reserve_out)
            / (reserve_in * fee_denominator + amount_after_fee);
        (output_amount > U256::ZERO).then_some((output, output_amount))
    }

    /// Applies a V2 Sync event: reserve0 and reserve1 are event data words.
    pub fn apply_sync(&mut self, reserve0: U256, reserve1: U256, block: u64) {
        self.reserve0 = reserve0;
        self.reserve1 = reserve1;
        self.block = block;
    }
}

#[derive(Debug, Default)]
pub struct ReserveCache {
    pools: HashMap<Address, PoolState>,
    last_block: u64,
}

impl ReserveCache {
    pub fn insert(&mut self, pool: PoolState) {
        self.last_block = self.last_block.max(pool.block);
        self.pools.insert(pool.pair, pool);
    }

    pub fn get(&self, pair: &Address) -> Option<&PoolState> {
        self.pools.get(pair)
    }

    pub fn get_mut(&mut self, pair: &Address) -> Option<&mut PoolState> {
        self.pools.get_mut(pair)
    }

    pub fn values(&self) -> impl Iterator<Item = &PoolState> {
        self.pools.values()
    }

    pub fn len(&self) -> usize { self.pools.len() }
    pub fn last_block(&self) -> u64 { self.last_block }

    /// Decode a canonical V2 Sync event without making an RPC call.
    pub fn apply_sync_log(&mut self, pair: Address, data: &[u8], block: u64) -> bool {
        if data.len() < 64 { return false; }
        let Some(pool) = self.pools.get_mut(&pair) else { return false; };
        // Ignore unconfirmed/out-of-order events rather than regressing reserves.
        if block == 0 || block < pool.block { return false; }
        pool.apply_sync(
            U256::from_be_slice(&data[..32]),
            U256::from_be_slice(&data[32..64]),
            block,
        );
        self.last_block = self.last_block.max(block);
        true
    }

    pub fn pairs(&self) -> impl Iterator<Item = &Address> {
        self.pools.keys()
    }
}

pub fn sync_topic() -> B256 {
    alloy::primitives::keccak256("Sync(uint112,uint112)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    #[test]
    fn rejects_stale_or_unconfirmed_sync_logs() {
        let a = address!("0000000000000000000000000000000000000001");
        let b = address!("0000000000000000000000000000000000000002");
        let pair = address!("0000000000000000000000000000000000000003");
        let mut cache = ReserveCache::default();
        cache.insert(PoolState {
            pair, token0: a, token1: b,
            reserve0: U256::from(100u64), reserve1: U256::from(200u64),
            fee_bps: 30, block: 20,
        });
        let mut data = [0u8; 64];
        data[31] = 15;
        data[63] = 25;
        assert!(!cache.apply_sync_log(pair, &data, 19));
        assert!(!cache.apply_sync_log(pair, &data, 0));
        assert_eq!(cache.get(&pair).unwrap().reserve0, U256::from(100u64));
        assert!(cache.apply_sync_log(pair, &data, 21));
        assert_eq!(cache.get(&pair).unwrap().reserve0, U256::from(15u64));
        assert_eq!(cache.last_block(), 21);
    }

    #[test]
    fn quotes_with_fee_and_updates_reserves() {
        let a = address!("0000000000000000000000000000000000000001");
        let b = address!("0000000000000000000000000000000000000002");
        let mut pool = PoolState {
            pair: address!("0000000000000000000000000000000000000003"),
            token0: a,
            token1: b,
            reserve0: U256::from(1_000_000u64),
            reserve1: U256::from(1_000_000u64),
            fee_bps: 30,
            block: 1,
        };
        let (_, out) = pool.quote(a, U256::from(10_000u64)).unwrap();
        assert!(out < U256::from(10_000u64));
        pool.apply_sync(U256::from(2u64), U256::from(3u64), 2);
        assert_eq!(pool.block, 2);
        assert_eq!(pool.reserve0, U256::from(2u64));
    }
}
