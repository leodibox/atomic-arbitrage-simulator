use crate::common::utils::bribe_from_revenue;
use alloy::primitives::U256;

pub struct Accounting {
    pub profit: U256,
    pub cost: U256,
}

impl Accounting {
    pub fn revenue(&self) -> Option<U256> {
        self.profit.checked_sub(self.cost).filter(|r| !r.is_zero())
    }
    pub fn bribe(&self, bribe_bps: u64) -> U256 {
        bribe_from_revenue(self.profit, bribe_bps)
    }
}

pub fn optimize_amount<F>(mut sim: F, lo: U256, hi: U256) -> Option<(U256, U256)>
where
    F: FnMut(U256) -> Option<U256>,
{
    let (mut lo, mut hi) = (lo, hi);
    let mut best = (U256::ZERO, U256::ZERO);
    for _ in 0..40 {
        if hi <= lo + U256::from(1u64) {
            break;
        }
        let span = hi - lo;
        let m1 = lo + span / U256::from(3u64);
        let m2 = hi - span / U256::from(3u64);
        let p1 = sim(m1).unwrap_or(U256::ZERO);
        let p2 = sim(m2).unwrap_or(U256::ZERO);
        if p1 > best.1 {
            best = (m1, p1);
        }
        if p2 > best.1 {
            best = (m2, p2);
        }
        if p1 < p2 {
            lo = m1;
        } else {
            hi = m2;
        }
    }
    (!best.1.is_zero()).then_some(best)
}
