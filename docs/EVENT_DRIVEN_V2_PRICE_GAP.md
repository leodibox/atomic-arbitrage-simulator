# Event-driven V2 price-gap net screening

## Objective

Find *potential* short-lived WETH/token price dislocations after canonical
Uniswap V2-style `Sync` events without launching services, subscribing to
new APIs, or adding code execution that sends transactions.

## Implemented on this branch

- The existing `subscribe_v2_sync` subscription already updates reserves
  in memory and triggers gross cycle scanning; it remains unchanged.
- A cheap local read of canonical, legacy V2-compatible, >=25 WETH
  pools groups them by their common intermediate token.
- For each pair of pools, integer reserve cross-products detect the
  absolute **marginal** price discrepancy in basis points. Using products
  of reserves is orientation/decimals independent.
- When a real `Sync` notification arrives, a gap >=65 bps triggers the
  existing *fresh block-pinned* read-only net scanner even if existing
  gross-size probes found zero cycles.
- Both existing event-triggered gross candidates and this new gap path
  are **bounded to at most one extra net screen per block**, with the
  previous five-second timeout and strict current-block check.
- No new RPC for quiet/synchronized markets, no background polling task,
  no third-party dependency, no new Render resource.
- Optimized two-hop loan sizing from #42 stays active; the ultimate
  decision is still reserved for Aave premium, slippage, gas and existing
  V2-only preflight. **No live execution code exists in this path.**

## Critical distinctions

A `Sync` event reflects reserve changes, which may arise from liquidity
changes as well as swaps. A marginal price gap is an **observation**, not
an assured profitable arbitrage route. Even >65 bps can be unprofitable
after finite-size impact, Aave, 10-bps slippage buffer and gas. Snapshot
latency, competing MEV searchers and changed state may invalidate it.

The net screen still only supports WETH V2 paths on Ethereum chain 1.
V3/Curve/Balancer swaps, pending transactions, private orderflow and
execution on Base are NOT added. The existing Ethereum $5 one-shot gas
sender guard, credentials, contracts and network are untouched.

## Safety, costs, acceptance

Before merging (Render automatically deploys `main`):

```powershell
cd "$HOME\Downloads\atomic-arbitrage-simulator"
git fetch origin
git switch feat/event-driven-v2-price-gap-20261121
git pull --ff-only
cargo test --locked --all-targets
```

GitHub Actions is unavailable due to an owner-confirmed billing hold.
Use the pre-existing, no-fee Windows unit-test evidence and the mandatory
Docker `RUN cargo test --locked --all-targets` build gate. No CI rerun.

On successful deploy, verify `arbitrage discovery scan ready` and
`read-only WETH V2 net screen` logs. If an eligible dislocation occurs,
look for `Sync event suggests V2 price movement; rechecking pinned net
economics (not a trade)`, then the subsequent pinned net scan showing
positive or rejected candidates. Do not mistake zero observed triggers
for broken code: threshold-crossing events can be rare.

Reference: https://developers.uniswap.org/docs/protocols/v2/concepts/pricing
