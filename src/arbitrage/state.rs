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

/// Uniswap V3 pool state (concentrated liquidity).
/// Quoting uses single-tick approximation (no tick crossing) which is
/// accurate for amounts small relative to liquidity.
#[derive(Debug, Clone)]
pub struct V3PoolState {
    pub pair: Address,
    pub token0: Address,
    pub token1: Address,
    /// sqrt(price) * 2^96
    pub sqrt_price_x96: U256,
    pub liquidity: U256,
    pub tick: i128,
    pub fee_bps: u64, // e.g. 5, 30, 100
    pub block: u64,
}

impl V3PoolState {
    /// Quote a swap using single-tick concentrated liquidity math.
    pub fn quote(&self, input: Address, amount: U256) -> Option<(Address, U256)> {
        if amount.is_zero() || self.liquidity.is_zero() || self.sqrt_price_x96.is_zero() {
            return None;
        }

        let zero_for_one = input == self.token0;
        let output_token = if zero_for_one { self.token1 } else { self.token0 };

        // Fee: amount * (1 - fee) 
        let fee_denom = U256::from(1_000_000u64);
        let fee_mult = fee_denom - U256::from(self.fee_bps * 100); // fee_bps in bps -> ppm
        let amount_after_fee = amount * fee_mult / fee_denom;

        // Single-tick swap:
        // For zero_for_one: price decreases
        // sqrtPrice' = (L * sqrtPrice) / (L + amount * sqrtPrice)  (simplified)
        // amountOut = L * (sqrtPrice - sqrtPrice') / 2^96
        // For one_for_zero: price increases
        // sqrtPrice' = sqrtPrice + amount * 2^96 / L
        // amountOut = L * (sqrtPrice' - sqrtPrice) / 2^96  (simplified)

        let q96 = U256::from(1u128) << 96;
        if zero_for_one {
            // token0 -> token1
            // new_sqrt_price = (L * sqrtP) / (L + amountIn * sqrtP / Q96) ... simplified
            // amountOut = L * (sqrtP - new_sqrtP) / Q96 * (1 - fee already applied)
            // Simplified single-tick:
            let numerator = self.liquidity * self.sqrt_price_x96;
            let denom: U256 = self.liquidity + (amount_after_fee * self.sqrt_price_x96 / q96);
            if denom.is_zero() { return None; }
            let new_sqrt = numerator / denom;
            if new_sqrt >= self.sqrt_price_x96 { return None; }
            let amount_out = self.liquidity * (self.sqrt_price_x96 - new_sqrt) / q96;
            (amount_out > U256::ZERO).then_some((output_token, amount_out))
        } else {
            // token1 -> token0
            let amount_in_scaled = amount_after_fee * q96 / self.liquidity;
            let new_sqrt = self.sqrt_price_x96 + amount_in_scaled;
            // Simplified: amountOut ≈ L * (1 - sqrtP/sqrtP') 
            let amount_out = self.liquidity * (new_sqrt - self.sqrt_price_x96) / new_sqrt * self.sqrt_price_x96 / q96;
            (amount_out > U256::ZERO).then_some((output_token, amount_out))
        }
    }
}

#[derive(Debug, Default)]
pub struct ReserveCache {
    pools: HashMap<Address, PoolState>,
    v3_pools: HashMap<Address, V3PoolState>,
    last_block: u64,
}

impl ReserveCache {
    pub fn insert(&mut self, pool: PoolState) {
        self.last_block = self.last_block.max(pool.block);
        self.pools.insert(pool.pair, pool);
    }

    pub fn insert_v3(&mut self, pool: V3PoolState) {
        self.last_block = self.last_block.max(pool.block);
        self.v3_pools.insert(pool.pair, pool);
    }

    pub fn get(&self, pair: &Address) -> Option<&PoolState> {
        self.pools.get(pair)
    }

    pub fn get_mut(&self, _pair: &Address) -> Option<&mut PoolState> {
        // Can't return mut from HashMap through &self; use get_v2_mut instead
        None
    }

    pub fn get_v2_mut(&mut self, pair: &Address) -> Option<&mut PoolState> {
        self.pools.get_mut(pair)
    }

    pub fn get_v3(&self, pair: &Address) -> Option<&V3PoolState> {
        self.v3_pools.get(pair)
    }

    pub fn get_v3_mut(&mut self, pair: &Address) -> Option<&mut V3PoolState> {
        self.v3_pools.get_mut(pair)
    }

    pub fn values(&self) -> impl Iterator<Item = &PoolState> {
        self.pools.values()
    }

    pub fn v3_values(&self) -> impl Iterator<Item = &V3PoolState> {
        self.v3_pools.values()
    }

    pub fn all_quotes(&self, input: Address, amount: U256) -> Vec<(Address, Address, U256)> {
        // Returns (pair, output_token, output_amount) for all pools that can quote
        let mut results = Vec::new();
        for pool in self.pools.values() {
            if let Some((out_token, out_amount)) = pool.quote(input, amount) {
                results.push((pool.pair, out_token, out_amount));
            }
        }
        for pool in self.v3_pools.values() {
            if let Some((out_token, out_amount)) = pool.quote(input, amount) {
                results.push((pool.pair, out_token, out_amount));
            }
        }
        results
    }

    pub fn len(&self) -> usize { self.pools.len() + self.v3_pools.len() }
    pub fn last_block(&self) -> u64 { self.last_block }

    /// Decode a canonical V2 Sync event without making an RPC call.
    pub fn apply_sync_log(&mut self, pair: Address, data: &[u8], block: u64) -> bool {
        if data.len() < 64 { return false; }
        if let Some(pool) = self.pools.get_mut(&pair) {
            pool.apply_sync(
                U256::from_be_slice(&data[..32]),
                U256::from_be_slice(&data[32..64]),
                block,
            );
            self.last_block = self.last_block.max(block);
            return true;
        }
        false
    }

    pub fn pairs(&self) -> impl Iterator<Item = &Address> {
        self.pools.keys().chain(self.v3_pools.keys())
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