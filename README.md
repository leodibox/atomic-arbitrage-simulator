# atomic-arbitrage-simulator
Ethereum atomic arbitrage simulator using Rust, Alloy, REVM, Foundry, and Aave V3 flash loans. Simulation-first, with profitability checks and secure execution.

## Known limitations

Aave mainnet fork tests require `HTTPS_URL` and are skipped when it is unavailable. Aave loan repayment remains unverified in CI runs without that RPC.
