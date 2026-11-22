//! Simulation-only atomic-arbitrage discovery.
//! This module deliberately has no signing or transaction-submission path.

pub mod candidate;
pub mod cycles;
pub mod metrics;
pub mod net;
pub mod gas_usd_diagnostic;
pub mod guarded_wiring;
pub mod live;
pub mod one_shot;
pub mod preflight;
pub mod rpc;
pub mod state;
pub mod topology;

pub use cycles::{find_cycles, Cycle, Hop};
pub use metrics::Metrics;
pub use state::{PoolState, ReserveCache};
