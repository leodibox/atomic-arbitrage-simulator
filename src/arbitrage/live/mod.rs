//! Live sender scaffolding for the arbitrage executor.
//! Additive only: nothing here signs or sends until the coordinator wires it.
pub mod activation;
pub mod coordinator;
pub mod quote;
pub mod reserve;
pub mod sender;
