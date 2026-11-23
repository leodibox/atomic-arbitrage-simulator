use alloy::primitives::{Address, U256};
use revm::primitives::Address as RAddress;

pub fn to_h160(a: Address) -> RAddress {
    RAddress::from_slice(a.as_slice())
}

pub fn is_weth(a: Address) -> bool {
    a == crate::common::constants::WETH
}

pub fn calculate_next_block_base_fee(gas_used: u64, gas_limit: u64, base_fee: u64) -> u64 {
    let target = gas_limit / 2;
    if gas_used == target {
        return base_fee;
    }
    if gas_used > target {
        let delta = (base_fee as u128 * (gas_used - target) as u128) / target as u128 / 8;
        base_fee + delta.max(1) as u64
    } else {
        let delta = (base_fee as u128 * (target - gas_used) as u128) / target as u128 / 8;
        base_fee.saturating_sub(delta as u64)
    }
}

pub fn bribe_from_revenue(revenue: U256, bribe_bps: u64) -> U256 {
    revenue * U256::from(bribe_bps) / U256::from(10_000)
}
