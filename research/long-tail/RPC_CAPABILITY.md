# Issue #15: RPC historical-access preflight (stacked research)

**Purpose:** decide whether the 83.58% missing 7-day event coverage in PR #16 comes from unsupported historical access or request-range limits **before** considering a subscription, extra RPC calls, or any sender/executor work.

The branch is based on PR #16's research head `59f33706337e104a4db013e0f46b4218f403ec9f`, **not** on production `main`. The accompanying PR targets `research/issue-15-long-tail-discovery-20261117` as a **DRAFT stacked PR**. It never changes that base branch.

## Evidence motivating diagnosis

PR #16 found 111 created pools/111 unique pairs in its accessible event windows (Uniswap V2: 85; Sushi V2: 0 in accessible ranges only; Uniswap V3: 26), but only 8,254/50,254 blocks per factory were covered (16.42%). The other **42,000 blocks per factory** (126,000 factory-blocks over 3 factories) were blocked in 63 factory×2,000-block windows with HTTP 403. No evidence establishes they lack events. The 74 observed WETH markets had no verified second usable pool; the 37 observed non-WETH pair-creation events also did not connect any **two observed WETH-created token markets** into a triangle. This latter check does not verify all historical counterpart pools or all DEX venues.

PR #16's 48 economic data points are limited to two control routes and 3 adjacent blocks. Its single positive **gross** roundtrip was negative after gas, and there is no certification of sustainable profitability.

## Read-only diagnostic

`src/rpc_capability.mjs` reads the existing checkpoint, verifies chain ID and the original final-block hash, locates the **last failed→first successful boundary** for one canonical factory, and sends at most four narrow event queries:

1. **One block in the known successful window** (provider/selector control)
2. **One block immediately before successful coverage** (historical-access check)
3. **Ten historical blocks** only if the old one-block succeeds
4. **The original failed window** only if the ten-block query succeeds

Including chain ID and pinned header, there are **at most six RPC requests**. Each failed request is recorded; no silent fallback, retries, 7-day backfill, or marking a missing range complete.

Output states:
- `SINGLE_OLD_BLOCK_ACCESS_DENIED`: one old block is rejected; shrinking window alone did not fix it. Could be authorization, pruning, a time gate or provider policy; do **not** assert which without provider confirmation.
- `RANGE_OR_PROVIDER_LIMIT_AT_TEN_BLOCKS` or `RANGE_OR_PROVIDER_LIMIT_AT_TWO_THOUSAND_BLOCKS`: narrower old query worked while a larger query did not.
- `OLD_WINDOW_NOW_AVAILABLE`: suggests earlier transient failure; **original checkpoint remains unchanged** and must be revalidated separately.
- `INCONCLUSIVE_RECENT_PROBE_FAILED`: even recent control query failed.
- Any mismatch in network, factory, coverage continuity or checkpoint hash fails closed.

## Run (only after installing dependencies)

From `research/long-tail`:

```sh
npm ci
node --test test/rpc_capability.test.mjs

# Configure HTTPS_URL privately outside source, shell history, PRs and logs.
DISCOVERY_CHECKPOINT=evidence/run-26437145-full/discovery.json \
  DIAG_FACTORY=UniswapV2 \
  node src/rpc_capability.mjs
```

Optional `DIAG_FACTORY=SushiV2` or `UniswapV3` to test their boundaries independently (each run up to 6 RPC requests), `DIAG_OUTPUT=/private/path/diagnostic.json` to keep output in a controlled file. In a proxy-restricted Codex runtime `NODE_USE_ENV_PROXY=1` may be necessary with normal TLS verification.

**Test status:** unit tests were authored, but cannot be claimed PASSED until Codex runs them. No Ethereum network diagnostic was run during PR creation.

## No-cost decision tree

- If historical **one-block** queries return HTTP 403, stop: a different provider / authenticated archive capability or independently indexed logs are needed; do not retry in a loop.
- If one block and ten blocks work but 2,000 fail, determine allowable range and exact pricing/quotas **before** designing a bounded, resumable backfill.
- A provider imposing 10 blocks/request would require **approximately 12,600 requests** solely for the 126,000 currently missing factory-blocks, before verification calls, assuming no overlap. Do not initiate automatically or assume free-tier allowance.
- On either route, retain per-factory block coverage and hashes, validate every imported pool against factory state on a consistent block, deduplicate logs by (txHash,logIndex), and report incomplete provenance honestly.
- Prefer **reusing the saved checkpoint** over paying for a new seven-day scan. If alternatives provide indexed logs, treat them as an *unverified discovery index* until the emitting factory, block, topics, and canonical pool address are independently checked. Indexed data is not a substitute for fork or gas validation.

No trade approval, no signer, no deployment, no Render, no edits to Rust or Solidity, no mergers, no secrets in output. Keep the base PR #16 and this PR in DRAFT. The go/no-go for an executor remains **NO-GO with current evidence**.
