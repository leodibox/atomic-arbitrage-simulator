# Intra-Uniswap topology extracted from PR #16 retained inventory

**As of 2026-11-17 — research-only addendum to PR #16, not new historical replay.**

## Source and reproducibility

- Source: `research/long-tail/evidence/run-26437145-full/inventory.json`, originally produced by PR #16.
- Ethereum Mainnet chain ID: **1**.
- End-of-block state **26437145**; hash `0x46eff74c6c5fdc7555c6f966f464352fcebf421e49dacb91575d48da13682dc9`.
- Derived output: [intra_uniswap_routes.json](evidence/run-26437145-full/intra_uniswap_routes.json).
- Pure logic: `src/intra_uniswap_routes.mjs`; bounded local-only CLI: `src/inspect_intra_uniswap.mjs`.
- Exact committed module and assertions: **9/9 checks passed** in a restricted V8 JavaScript execution using the uploaded repository content, including a fixed test fixture for canonical V2 fee (30bp). **The official Node test runner has not yet been executed for this new suite.** Codex should execute the Node suite before treating it as Node-validated.

From `research/long-tail`:

```sh
node --test test/intra_uniswap_routes.test.mjs
node src/inspect_intra_uniswap.mjs \
  evidence/run-26437145-full/inventory.json \
  evidence/run-26437145-full/intra_uniswap_routes-recomputed.json
```

Review the JSON fields and verify counts, block/hash and route identities match the checked-in evidence. This command makes **zero RPC calls** and never signs/sends.

## Observed topology (no profitability calculation)

| Item | Count |
| --- | ---: |
| Canonical Uniswap pools with positive local state from retained inventory | **74** |
| Uniswap V3 2-hop routes crossing **distinct V3 fee-tier pools**, direction-sensitive | **12** |
| Uniswap V2 ↔ Uniswap V3 2-hop routes, direction-sensitive | **8** |
| Intra-Uniswap closed 3-hop triangles represented in this inventory | **0** |
| Total unique directional intra-Uniswap routes | **20** |
| Non-Uniswap pool observations excluded | **1** (Sushi control) |
| Candidate routes with a verified net positive P&L | **0** |

The control USDC/WETH has one Uniswap V2 and four Uniswap V3 fee-tier pools (100, 500, 3000, 10000 ppm), making **12** directed distinct-pool V3↔V3 and **8** directed V2↔V3 routes. No same-pool round-trip is counted as arbitrage; these are distinct pools. Total fees are shown only as an approximate sum in basis points, **not** executed round-trip price impact, net profit, tip, builder payment, flash-loan premium or inclusion success.

Other long-tail markets in the retained inventory have single verified usable Uniswap pool observations. This inventory was built around WETH/token pairs and does **not** establish that Ethereum lacks triangles involving non-WETH intermediate pools, or that no other venues/tokens can be profitable. The record excludes unverified or zero-active-liquidity observations. Conflicting verified identity records for one pool now fail closed rather than choosing one.

## What still must be measured

1. Obtain an authenticated / independently verifiable **event-conditioned Swap source**, including trigger transaction hash/index, receipt, block hash, USD value with independent price provenance, pool fee and liquidity band. Follow the pre-registered selection rules from `TRANSACTION_TRIGGERED_EXPERIMENT.md` and preserve separate blue-chip/long-tail strata.
2. **Replay state after the entire triggering transaction**, not just its Swap log and not the post-block inventory. A new transaction cannot execute between internal calls of one atomic transaction. Missing trace capability is `INSUFFICIENT_REPLAY_ACCESS`, not zero profit.
3. For the prioritized intra-Uniswap routes, compute exact tick-aware V3 quotes and all pool state at that transaction boundary, and then simulate a full Morpho-funded settlement without synthetic liquidity changes.
4. Price **cold measured gas** against that block's base fee plus explicit tip and builder/relay payment scenario, avoiding double counting. Apply risk and failed-attempt sensitivity. Calculate net-positive *simulated* opportunity with an identifying block/tx/amount, or report that none is verified.
5. Any positive simulation is only an opportunity after modeled fees. Positive realized expected value and successful inclusion cannot be certified without own inclusion/failure observations and complete wallet reconciliation.

Public historical RPC tested in PR #17 rejected one older `eth_getLogs` block and separate historical `eth_call` and `eth_getStorageAt` reads with HTTP 403. Do not pretend this file has solved that, or fund production execution based on 20 candidate topologies.

## Outcome

**INCONCLUSIVE for transaction-conditioned profitability. NO-GO for real trading/sender.**

The topology module materially expands the scanner's **research universe** beyond Uniswap V2/Sushi V2, with deterministic route enumeration and validation, without changing the Rust scanner, contracts, Render, PR #10-#14 or live money.
