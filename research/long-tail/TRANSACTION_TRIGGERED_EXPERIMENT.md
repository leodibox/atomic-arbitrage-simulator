# Issue #15 — pre-registered transaction-conditioned replay

**Amendment 2026-11-17. This supersedes broad seven-day pool creation coverage as the next experimental priority.** PR #16's recorded NO-GO remains the correct conclusion *for its block-end sample only*. We do not overwrite or relabel its existing evidence. This document is a research protocol, **not a claim that new swaps have been analyzed or that a profitable trade was found**.

## Why change the sample

- A finalized-block `eth_call` reads end-of-block state. It cannot show a transient price gap created by transaction A and eliminated by later transaction B **inside the same block**. PR #16's three observation blocks are adjacent; that is not equivalent to three independent volatility events.
- A `Swap` log records a pool's in-transaction post-swap state, but **a new arbitrage transaction cannot be inserted between calls in the same atomic transaction**. The correct counterfactual checkpoint is **after the entire triggering transaction**, not after one internal Swap event.
- Historical access to log data alone is not historical EVM state replay. For verified results, obtain the parent block state and replay preceding transactions in canonical order, or use a provider's independently verified transaction-position tracing/state capability. Use a fork on an exact block+transaction-position, not only a fork at block end.
- A reconstructed inter-transaction profitable opportunity is a **counterfactual**, not an included trade or verified inclusion probability. Only a realistically executable inclusion mechanism and own observations can calibrate positive *expected value* (EV).

## Sampling design — frozen before inspecting results

- Target period: **30 days preceding a pinned finalized end header**; record start/end timestamps, block numbers and hashes.
- Two non-interchangeable strata, **up to 100 distinct blocks each**: (A) blue-chip pools with >= $100k independently verified *per-side* depth; (B) long-tail token pairs with >=2 independently confirmed venues, >= $50k per-side depth, token age <= 30 days, and explicit transfer/FOT/honeypot status. Missing safety evidence = `UNKNOWN`, never `SAFE`.
- Identify eligible `Swap` events via authenticated or reproducibly indexed sources. Select the **largest single swap (independently priced USD notional)** per block for each stratum; rank descending with deterministic tie breakers. Do not let one block count twice across strata. Normalize USD notional against a defensible reference *independent of the candidate pool* and record quote source + timestamp.
- Avoid selection on future profitability, sampled route results or the desired outcome. Keep source metadata, excluded/missing blocks, duplicate events and conflicts.
- Long-tail opportunities need **two distinct pools or a proven triangle**, with all fees and factories verified; a mere new token/pool is not a second venue. Do not manufacture a top-100 if insufficient eligible observations; mark `INSUFFICIENT_SAMPLE`.
- Pool inventory may come from an index instead of repeated archive-wide `eth_getLogs`; verify transaction hash/logIndex/block hash and emitting contract on chain for chosen samples. Do not treat an index's post-swap state as a transaction-position trace.

## Exact replay and P&L

For each sampled triggering transaction:
1. Verify chain 1, parent header, selected block hash, trigger tx hash, transaction index, receipt and the triggering pool's event. Reject inconsistencies.
2. Reconstruct state **after the complete trigger transaction** and before the next transaction. If trace/replay/history is unavailable, mark `NOT_REPLAYED`, not negative. Do not insert a candidate between events in one atomic router transaction. Show whether a hypothetical next-transaction slot is reachable; actual builder inclusion is unproven.
3. Obtain all venues' reserves/slot0/initialized ticks at *that same transaction position*; calculate prices/routes and filter fee-adjusted marginal spreads. A `slot0` from block end is not acceptable as the other venue's state here.
4. Size with bounded search and tick-aware exact quotes; validate candidate WETH→...→WETH settlement in a fork. Check real token balance deltas, funding, Morpho callback, failure/partial fill, caps and FOT/blacklist risk. No synthetic reserve manipulation and no arbitrary private-token approvals.
5. Measure cold path gas for the exact research route and **price it with the selected historical block's `baseFeePerGas`** plus an explicitly sourced or conservative inclusion tip; report `gas_total_wei = gas_units × (baseFee + tip)`. Distinguish EIP-1559 counterfactual price from the realized price of another transaction. Include any additional relay/private-builder fee and failed-attempt cost.
6. Store `gross_wei`, `post_flash_fee_wei`, `post_gas_wei`, reserve depth, amount, price impact, %/bp on principal, verified/unverified flags and a trace/fork evidence pointer. Calculate a realistic break-even size only if a positive segment exists.
7. Report marginal gross margin percentiles (p50/p90/p99), positive-after-gas hit rates, median positive size, median positive margin, notional/profit distributions, missing-data proportion, risk reject reasons, and provenance by stratum (depth, fee, token age, source venue, block time).

## Pre-registered interpretation (NOT a profitability guarantee)

- `GO_TO_MORE_RESEARCH` only if **at least two distinct transactions** pass the exact state/quote/funding/settlement/gas checks with positive *net simulated margin*, and token risk is sufficiently characterized. This still does **not** authorize trading, prove a positive realized EV or approve an executor/sender.
- `STOP_THIS_STRATUM` only with at least 90 independently replayed, eligible event-triggered transactions in that stratum and zero positive net opportunities **after** cold execution gas at historical block fees, and sufficient independent source/coverage quality. Otherwise `INCONCLUSIVE`. The stop is a bounded prioritization decision; it is not a universal claim about Mainnet.
- At 0 positive out of 200 independent samples the approximate one-sided 95% upper bound on the conditional event hit rate is **1.49%**, **not below 1%**. Dependence between blocks can make this bound optimistic. Do not claim a statistical <1% rate from 0/200.
- Positive-net **EV** cannot be certified from a historical fork alone: let `p_success` remain `UNKNOWN` until calibrated with this bot's own attempted inclusion/reversion data. Scenario EV may be shown, explicitly hypothetical: `EV(p) = p × (gross - funding - success_gas - other_cost) - (1-p) × failure_gas`.
- If historical state is denied (e.g. HTTP 403), `INSUFFICIENT_REPLAY_ACCESS` takes precedence: report swap indices/availability, do not fill failed blocks with zero opportunity. PR #17's RPC diagnosis is independent and **must not gate** moving the sampling design forward.

## Limits

Read-only/instrumentation only. **No signatures, transaction broadcast, private keys, bundles, deployment, unpause, purchases, or real-money actions.** Keep PR #16 DRAFT and prior evidence immutable. Never assert a profitable trade absent a block hash, trigger tx index/hash, exact transaction-position state, settled post-gas margin, and evidence of an actual feasible ordering boundary.
