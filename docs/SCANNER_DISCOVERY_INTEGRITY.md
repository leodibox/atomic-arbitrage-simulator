# Scanner discovery integrity — candidate accounting, not execution authorization

## What this draft changes

This PR only changes `src/main.rs`. It is intentionally separate from Aave executor PR #10 and research PRs #11/#12.

- `POOL_SCAN_ENABLED` defaults to **false** (read-only allowlisted discovery). The legacy pending-transaction strategy requires explicit literal `true`; invalid values fail closed instead of silently activating it. This guards the omitted-variable failure mode. **Do not set this variable to true when configuring the arbitrage scanner.**
- Gross discovery cycles are deduplicated by **start token and ordered (pool, token-in, token-out) route**. Reverse-direction routes remain distinct. Among multiple probed amounts on the *same* path, retain the largest gross profit, not the first one encountered.
- Keep the historical `cycles` discovery metric for compatibility, but add `v2_compatible_gross_candidates` and `incompatible_gross_candidates`. These counts are **gross only** and do not authorize sending. A route with a V3 hop, unknown V2 pool, or a non-30bps V2 fee is not compatible with the current V2-only receiver.
- Existing per-cycle `net_profit_verified=false`, `configured_gas_cost_wei_unverified` and warnings stay unchanged; we do not claim that a candidate can cover gas.
- Rust regression tests cover fail-closed scan mode, keeping both route directions, and replacing an early suboptimal amount with the later better probe.

## What this draft does NOT change

- Does not modify `find_cycles`, pool math, preflight, Solidity contracts, private key handling, or Render configuration.
- Does not wire `prepare_v2_flash_loan` without a *genuine* `GasQuote`. Because gas is in ETH wei and the start loan token can be USDC or WETH, the previous first-hop reserve-ratio conversion was unsafe. Do **not** invent a gas quote to make preflight pass.
- Does not implement size optimization or newly discovered tokens: those are independent tasks pending real observations and gas accounting.
- Does not merge or deploy, sign or submit transactions, unpause contracts, or authorize spending.

## Acceptance tests (Codex / trusted Rust development machine)

```sh
cargo test --locked --all-targets
cargo build --release --locked
```

Verify these tests pass on the branch and that the production scanner stays read-only when `POOL_SCAN_ENABLED=false`. Do not report a test as passed unless it was actually executed. GitHub Actions is blocked by account billing; there is no Rust compiler in the current chat container, so tests are **not yet executed** here.

## Safety and follow-up

1. Privately verify real executable gas cost `eth_estimateGas` and chain/asset conversion; only then integrate `prepare_v2_flash_loan` with a **read-only**, fail-closed diagnostic branch; never use mocked zero gas.
2. Add explicit per-asset minimum net profits (USDC 6 decimals vs WETH 18) and a bounded, deterministic size optimization. Count approved-for-execution candidates separately from gross discovery.
3. Keep V3-only routes unexecutable until a separately reviewed V3 receiver is actually implemented and tested; never pass V3 pools as V2 calldata.
4. Preserve singleton nonce fencing, daily gas/loss limits and direct on-chain simulation before any future authorized live transaction.

No production changes are approved by this draft.
