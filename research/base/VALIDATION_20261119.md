# Base viability validation update — 2026-11-19 UTC

## Confirmed research observations

- Candidate A Uniswap V3 WETH/USDC 0.05% pool `0xd0b53d9277642d899df5c87a3966a349a798f224` identified from [GeckoTerminal](https://www.geckoterminal.com/base/pools/0xd0b53d9277642d899df5c87a3966a349a798f224).
- Candidate A SushiSwap V3 WETH/USDC 0.05% pool `0x57713f7716e0b0f65ec116912f834e49805480d2` identified from [Sushi's official pool page](https://www.sushi.com/base/pool/v3/0x57713f7716e0b0f65ec116912f834e49805480d2/smart/0xe51c514fefcfeb28cc78a83b0ba954baa2c38a12) and [GeckoTerminal](https://www.geckoterminal.com/base/pools/0x57713f7716e0b0f65ec116912f834e49805480d2).
- Market page indexed values in this review: Sushi pool approximately USD 47k TVL, versus Uniswap V3 0.05% WETH/USDC tens of millions USD. These are **off-chain provider snapshots**, not authenticated block-pinned reserves, and must never be treated as available execution liquidity.
- Sushi also lists a Base 0.01% WETH/USDC pool `0x482fe995c4a52bc79271ab29a53591363ee30a89` from [GeckoTerminal](https://www.geckoterminal.com/base/pools/0x482fe995c4a52bc79271ab29a53591363ee30a89). It is a **separate candidate to verify**, not automatically a better route.
- The original Candidate A combined fee hurdle is 10bp, before price impact, flash borrowing and Base L2 + L1 data fees. Smaller venue depth sharply limits theoretical executable size.

## Local test evidence — NOT on-chain observations

The two scripts `research/base/verify_pools.mjs` and `research/base/verify_pools.test.mjs` were transcribed from the GitHub branch into a Node v22.16.0 container and run with `node --test` on 2026-11-19 UTC. Result: **4 passed, 0 failed**, covering strict ABI word decoding, metadata rejection, wrong chain abort and same-block mock reads without `eth_sendTransaction`/`eth_sendRawTransaction`. These are entirely **mock-backed tests**. The public Base RPC could not be queried in that container because outbound DNS resolution is disabled; external pool verification is still incomplete.

## STOP / GO

Status: **NO-GO FOR LIVE EXECUTION; BASE ECONOMICS NOT YET EVALUATED**. No profitable executable route, historical replay, real gas accounting, competition-adjusted proceeds, signing, deployment, or funds movement. Require verified same-block canonical pool identities and exact quoter calculations before proceeding with the bounded 2,000-event evaluation. Keep PR #19 in DRAFT and do not merge to auto-deploying main.
