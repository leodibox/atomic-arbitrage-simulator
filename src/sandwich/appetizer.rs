use alloy::primitives::U256;

pub fn is_appetizing<F>(mut sim: F, max_gas_cost: U256) -> bool
where
    F: FnMut(U256) -> Option<U256>,
{
    let probe = U256::from(100_000_000_000_000_000u64);
    match sim(probe) {
        Some(profit) => profit > max_gas_cost,
        None => false,
    }
}
