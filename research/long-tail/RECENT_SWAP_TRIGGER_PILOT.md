# Read-only recent Swap transaction candidate pilot — PR #16

## Scope and current result

This stage adds actual research code for **recent finalized Swap event selection** on the five existing canonical Uniswap USDC/WETH pools. It is the candidate-finding precursor to the preregistered **transaction-position replay**, not the replay itself.

**Status 2026-11-17:** pure JavaScript committed assertions **11/11 passed** when run in a restricted V8 environment against the exact GitHub source; **official Node `node --test` and a live RPC run have NOT been executed**. Do not claim new Swap transactions, a profitable trade, a positive EV, or improved long-tail coverage yet.

Existing PR #16 report remains unchanged: no verified positive-net trade, no sender, and prior public archive RPC returned HTTP 403 for older logs and state. No secrets, signing, or funds are involved.

## Implementation

- `src/swap_trigger_candidates.mjs`: validates canonical Uniswap V2/V3 pool identity from old inventory, decodes raw V2 Swap amounts and **signed** V3 Swap deltas, computes the WETH-side movement, deduplicates events, selects **one largest Swap per distinct block** and verifies its transaction receipt and block header (including historical base fee). Rejects malformed events, spoofed factory metadata, conflicting pool records and tampered receipt data.
- `test/swap_trigger_candidates.test.mjs`: eleven tests including negative V3 delta, duplicate logs, block bounds, altered receipts, reverted transactions and missing gas fee.
- `src/run_recent_swap_sample.mjs`: read-only bounded live sampler. Validates `chainId = 1`, verifies the saved historical inventory block hash, pins a **fresh finalized block**, gets `Swap` logs for the five known Uniswap pools, then requests receipts and block headers only for a small ranked shortlist. It never sends transactions.
- Old pool identity is used as a candidate index, **not** a guarantee that this is a complete, current market inventory. Status flags explicitly record missing USD depth, actual token safety, transaction-position replay, realized cold gas, builder payments and net positive economics.

The pilot ranks by **absolute WETH movement**, not independently verified USD notional, and it is restricted to the known USDC/WETH control. Thus it is an instrumentation and data-access pilot **only**, not the preregistered 30-day/200-block high-volatility sample or the long-tail stratum.

## Exact Codex validation

From the branch `research/issue-15-long-tail-discovery-20261117`:

```sh
cd research/long-tail
npm ci
node --test test/intra_uniswap_routes.test.mjs
node --test test/swap_trigger_candidates.test.mjs
```

Then, **only if** Codex can obtain a verified bounded public or previously configured RPC endpoint, run a capped read-only pilot (the built-in public fallback is `https://ethereum-rpc.publicnode.com`):

```sh
SWAP_WINDOW_BLOCKS=48 SWAP_MAX_CANDIDATES=8 \
  SAMPLE_OUTPUT=evidence/recent-swap-sample-20261117.json \
  node src/run_recent_swap_sample.mjs
```

In an environment requiring the normal HTTPS proxy, `NODE_USE_ENV_PROXY=1` may be needed; do not disable TLS. Optional `HTTPS_URL` must be configured privately; never paste credentials into chat, GitHub or logs.

The runner makes **at most 20 read-only RPC calls**: chain ID, original inventory header, finalized header, one filtered logs query, and up to 8 × (receipt + block header). It scans at most **48 blocks by default** (configurable between 1–128), rejects over 5,000 log entries, and selects at most eight candidates. No retry loop or massive historical scan. If the public provider rejects the request or the required data is inaccessible, report `BLOCKED` and the exact method/status. Do not replace the request with `latest` nor silently widen the scan. The runner should not be given more than a minimal capped test budget without user approval.

## Acceptance: discovery ≠ successful arbitrage

For each selected event record:
- block number/hash/timestamp and **historical baseFeePerGas**
- pool address/fee, WETH amount moved, transaction hash/index, event log index
- receipt status and confirmation that the exact event appears in the receipt
- flags `postTransactionStateVerified=false`, `netProfitVerified=false`, `tradeExecutionAllowed=false`

**Next experiment** requires state **immediately AFTER the complete triggering transaction**, rather than post-block reserves. The current public historical RPC did not provide transaction-position state. A trace or deterministically reconstructed parent-block replay is required, with evidence of canonical ordering. Only then can the candidate be quoted, sized and settled in a fork and marked positive simulated net after that block's base fee, realistic builder payment and gas.

**GO remains closed** until the existing transaction-conditioned preregistration requirements are met. No new PR, contract deployment, sender/relay work, key, or paid service is authorized by this pilot.
