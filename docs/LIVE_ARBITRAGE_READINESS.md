# Fastest safe route from simulation to live atomic arbitrage

## What is built
- The worker runs on Render and receives Ethereum mainnet block and pool reserve updates.
- Pool discovery/quoting includes V2-compatible pairs and experimental V3 quotes. V3 quotes currently use a single-tick approximation and are **not** a validated execution price; do not use them as an exact price or for sending real transactions.
- `FlashLoanArb.sol` exists, with simple Aave V3 flash borrowing, callback, multi-hop V2 swaps, and a minimum asset-denominated profit check.
- The **current Rust worker cannot sign or submit arbitrage transactions**, regardless of `DEBUG`, `POOL_SCAN_ENABLED`, or `SIM_AMOUNT_MODE`.
- This PR optimizes **discovery only**. It does not enable live execution and does not create on-chain transactions.

## Missed-opportunity fixes introduced here
1. Process V2 `Sync` reserve events immediately rather than waiting up to `SIM_INTERVAL_SECS` (periodic RPC reconciliation remains). V3 still uses periodic refresh and needs properly decoded Swap events for equivalent event-driven coverage.
2. Probe multiple correctly scaled notionals for each start token; `SIM_AMOUNT_MODE=auto` by default, even if legacy `SIM_AMOUNT_RAW` is set.
3. Verify on-chain V2 and V3 pool token ordering and log a structural topology check. If no closed routes exist for `SIM_MAX_HOPS`, **no amount of waiting or faster event processing will generate a cycle**. Verify and widen the vetted allowlist only then.
4. Unit tests cover token decimal sizing, impossible/possible 2–3-hop pool routes, invalid V3 quote inputs and stale V2 Sync events.

## Remaining blockers before anyone can truthfully call this "live"
- [ ] CI or local `cargo test --locked --all-targets` passes for this discovery PR. Tests have **not** been run by ChatGPT.
- [ ] Inspect authenticated Render startup logs for `structural_routes` and `amount_mode=auto` after deploying reviewed code.
- [ ] Compare expected net proceeds with *actual* Aave premium, swap fees, slippage, gas estimate/priority fee (gas is paid in ETH, not automatically in the start token), transaction failure costs, and stale-state probability. Today's `SIM_GAS_COST_RAW=0` does NOT satisfy this.
- [ ] Build an off-chain Rust dispatcher that verifies approved chain, allowlisted contract and token paths, on-chain reserves/freshness, profitability, estimated gas, available capital and nonce before signing.
- [ ] Harden and audit `FlashLoanArb.sol`: check route input/output token matching, use checked ERC-20 transfers/approvals, nonzero addresses, configurable emergency pause, max path size, test fee-on-transfer/rebasing/nonstandard ERC-20 behavior, reentrancy, and enforce time-sensitive bounds. Existing tests are not a substitute for a security review.
- [ ] Use an approved private transaction delivery route where available; simulate exact calldata against current/fork state before signing, enforce short transaction expiry / block validity, and account for competition and failed inclusion.
- [ ] Deploy or verify a trusted executable on Ethereum only after deterministic mainnet-fork tests, unit/integration/fuzz tests, explicit spend budget, and independent review.
- [ ] Add fail-closed LIVE switch, strict gas/ETH loss budget, transaction-revert limits, kill switch, nonce management, monitoring, and reconciliation of realized P&L.
- [ ] Obtain operator's explicit approval before using a wallet, deploying contracts, making paid RPC commitments, or submitting transactions.

## Operating rules
1. **Do not** flip `DEBUG=false` assuming this enables trading.
2. **Do not** flip `POOL_SCAN_ENABLED=true` assuming this enables trading. The alternative code path is a different simulation/pending-tx strategy.
3. **Do not** fund a wallet or insert `PRIVATE_KEY` while only deploying this discovery fix.
4. After code review and successful tests, deploy discovery first. That can reduce missed *signals* without risking capital; it does not capture them.
5. A limited live canary is a distinct deployment/release milestone, never a toggle-only shortcut.
