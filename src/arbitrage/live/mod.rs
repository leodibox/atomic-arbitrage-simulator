//! Live sender scaffolding for the arbitrage executor.
//! Additive only: nothing here signs or sends until the coordinator wires it.
pub mod quote;
pub mod reserve;
pub mod activation;
pub mod sender;
pub mod coordinator;
