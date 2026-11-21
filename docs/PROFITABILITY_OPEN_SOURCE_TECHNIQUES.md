# Actionable MEV bot research (2026-11-20) — implement value, not endless tests

## Current economic evidence

Owner-run Ethereum mainnet 80/80 V2↔V3 quotes and Base 16/16
Aerodrome↔Uniswap V3 quotes all found **zero positive after assumed
flash fees before gas** in sampled blocks. Existing V2-only mainnet
worker is read-only and has no proven net-positive route.

The main gap that can be fixed **without new infrastructure or buying
feeds** is *trade sizing*. Fixed loan amounts (10 samples) may miss
the best achievable return on a short-lived V2/V2 dislocation.

## Immediate implementation

`src/arbitrage/net.rs` computes an approximate **profit-maximizing
WETH loan amount** for each ordered two-pool V2/V2 route sharing an
intermediate asset, from onchain-verified reserves already in memory.

- Integer ternary search on the actual fee-aware
  `PoolState.quote` function (max 128 rounds, 8 integer neighbors).
- No third-party crate, API token, subscription, signer, transaction,
  extra RPC, changed loan cap or changes to executor contracts.
- Preserves **all existing 10 sample sizes** and adds only positive-gross
  optimum sizes within the *existing* max flash loan.
- All discovered routes still pass existing same-block, Aave fee,
  gas, haircut, net-profit minimum, and **V2-only** preflight gates.
- The optimum is a **quote**, not executable proof; integer rounding,
  stale blocks, MEV competition and state changes can invalidate it.
- A small deterministic regression uses constant-product reserve data
  to show the optimizer finds a gross-positive amount below the cap.

This optimization is limited to constant-product V2/V2 hops.
**Never apply this model to V3, V4, Curve or stable pools**.
Event-triggered net screening (#41) already exists in main.

## Open-source / official comparison

| Source | Verified useful technique | Decision |
|---|---|---|
| Uniswap V2 pricing: https://developers.uniswap.org/docs/protocols/v2/concepts/pricing | Fee-aware, reserve-dependent swap size | Implement now, reuse internal PoolState |
| degenbot-solvers: https://docs.rs/crate/degenbot-solvers/latest | Multi-hop input optimization, closed form or search | Adapt idea only; pre-release; no new dependencies |
| Artemis (Paradigm): https://github.com/paradigmxyz/artemis | Collector / Strategy / Executor separation | Existing scanner already event-driven, do not rewrite |
| Tycho: https://github.com/propeller-heads/tycho | Indexed multi-DEX states and streaming updates | Candidate for later *if* measurable opportunity volume; new releases have Fynd license, review first |
| REVM: https://github.com/bluealloy/revm | Local exact EVM transaction simulation | Only when a real positive candidate appears; no point simulating zero-positive spreads |
| Flashbots MEV-Share tutorial: https://docs.flashbots.net/flashbots-mev-share/searchers/tutorials/flash-loan-arbitrage/bot | Explicit-event backrun workflow / conditional bundles | Research option if confirmed viable spreads; do not connect a sender yet |
| Uniswap V4 flash accounting: https://developers.uniswap.org/docs/protocols/v4/guides/flash-accounting | Lower transfer overhead for V4 routes | Deferred, unsupported by current V2-only executor |

Third-party repositories should be audited for license, maintenance and
security before copying code. **No third-party code was copied**.

## No-go vs. progression

1. Do not merge PR unless `cargo test --locked --all-targets` passes.
2. Keep existing Ethereum $5 *sender* hard cap, execution disabled.
3. If post-merge observations find zero positive routes, stop adding
   optimizers and audit real opportunity frequency / subscription cost
   before spending more.
4. If a net-positive candidate appears: exact same-block fork replay,
   actual contract gas, fee and preflight; only after validation and
   explicit execution approval consider sending.
5. No claim of money made or return guarantee.

## Local validation (Windows PowerShell)

```powershell
cd "$HOME\Downloads\atomic-arbitrage-simulator"
git fetch origin
git switch feat/optimal-v2-size-screen-20261120
git pull --ff-only origin feat/optimal-v2-size-screen-20261120
cargo test --locked --all-targets
```

The PR is initially draft, and Render auto-deploys commits to main,
so **do not merge before reviewing the test result and deploy impact**.
