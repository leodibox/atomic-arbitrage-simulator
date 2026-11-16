# Live Aave arbitrage — executable route preflight (implementation milestone)

**Read this before configuring a wallet or sending any transactions.**

The Rust scanner recognizes V2 and V3 AMM pool paths. The existing Solidity `contracts/src/FlashLoanArb.sol` executes **V2-style pairs only**, with 30-basis-point V2 swap math. It has **not** been independently audited or verified as deployed on Ethereum mainnet. The scanner's single-tick V3 local quotes must not be used as transaction execution prices.

## What this milestone adds

`src/arbitrage/preflight.rs` contains a deterministic, network-free transaction *planner*. It validates a proposed V2-only cycle against the current reserve cache and prepares the Solidity `executeArbitrage(address,uint256,bytes,uint256)` calldata. The planner refuses routes when:
- Any hop is unknown, V3-based, misordered, repeated, or does not exactly match the pool's local quote.
- Any pair has fee different from the contract's hardcoded 30 bps.
- Pool block is stale or appears ahead of the current block.
- The amount, route length, gas-spend cap, or required profit policy is violated.
- An explicit gas cost **in ETH and in the start asset** is missing, zero, or from an older block.
- Expected gross proceeds cannot cover the conservatively rounded Aave premium, asset-denominated gas reserve, and minimum surplus.

**No transaction signing, RPC sending, wallet key usage, spending, contract deployment, or Render secret changes are added.** The existing worker remains a discovery scanner.

## What still blocks a live-money launch

1. **Independent contract review:** harden the Aave flash-loan receiver, enforce each pair's token0/token1 and the actual input token, check ERC20 transfer/approve return values, emergency pause, maximum hops, on-chain minimum profit, and stronger role controls; test with Foundry and mainnet forks.
2. **Verified live deployment:** inspect Ethereum chain 1 deployed bytecode, owner, recipient, Aave Pool, allowed assets, fees, and a dedicated capital-limited signer. Never post or commit any private key.
3. **Accurate execution pricing:** reserve cache is a discovery heuristic, not a live price guarantee. Verify a fresh block-specific V2 route quote and dynamic Aave premium; V3 execution requires a separate contract/adapter.
4. **Trusted gas conversion:** estimate the transaction's gas at current base/priority fees, calculate actual ETH spending, and conservatively value it in the loan asset using an independent reference price. The existing `SIM_GAS_COST_RAW` first-hop reserve-ratio shortcut cannot fulfill this.
5. **Pre-submit exact simulation:** `eth_call` the final calldata against the *actual deployed contract*, and `eth_estimateGas` at the current state. Re-run after changes; reject reverts and stale state.
6. **Signer and dispatch:** dedicated permissioned key, chain and contract allowlist, nonce serialization across instances, private relay when appropriate, expiry / max fee policy, receipt tracking, real gas and asset P&L accounting, failure budget, kill switch, and explicit activation.
7. **Singleton guarantee:** prevent duplicate live worker instances from sending transactions (durable leader election / nonce fencing); Render logs recently contained overlapping scanner instance IDs.
8. **Security review and capped canary:** independent contract and signer review, Foundry fork/invariant/fuzz tests, signed owner approval of maximum ETH gas loss, maximum loan size, maximum daily losses, and a separate live canary before scaling.

Do **not** set `DEBUG=false`, `POOL_SCAN_ENABLED=true`, or introduce a wallet key to turn on live trading. There is no live signing/submission path in this milestone.

## Verification

- Rust: `cargo test --locked --all-targets`
- Contract tests (when hardened): `cd contracts && forge test`
- Review the actual Render deploy logs for `test result: ok`; do not claim tests passed just because GitHub merged.
- Check live logs for `fresh=3 stale=0` when all 3 monitored pools are truly fresh; check separately that the V3 pool data is still current.

The goal is a *real*, auditable live-money executor, not counting false positive simulated cycles as profits.
