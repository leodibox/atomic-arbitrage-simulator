use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct Metrics {
    pub candidates: AtomicU64,
    pub simulations: AtomicU64,
    pub profitable: AtomicU64,
    pub rejected: AtomicU64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MetricsSnapshot {
    pub candidates: u64,
    pub simulations: u64,
    pub profitable: u64,
    pub rejected: u64,
}

impl Metrics {
    pub fn candidate(&self) { self.candidates.fetch_add(1, Ordering::Relaxed); }
    pub fn simulation(&self) { self.simulations.fetch_add(1, Ordering::Relaxed); }
    pub fn profitable(&self) { self.profitable.fetch_add(1, Ordering::Relaxed); }
    pub fn rejected(&self) { self.rejected.fetch_add(1, Ordering::Relaxed); }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            candidates: self.candidates.load(Ordering::Relaxed),
            simulations: self.simulations.load(Ordering::Relaxed),
            profitable: self.profitable.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
        }
    }
}
