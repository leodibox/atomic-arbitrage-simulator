# Base viability gate — read-only, zero new infrastructure

Goal: determine whether **Base chain 8453** has reproducible, potentially profitable **one specific two-pool route family** using existing scanner components. This is NOT a claim that switching chain creates profit.

## Baseline verified in source
- Production `main` is Ethereum-only. `src/common/constants.rs` accepts chain IDs 1/11155111 and hard-codes Ethereum factory, WETH and pool scan start block.
- `src/main.rs` always connects via WebSocket before choosing scanner mode. The official public Base RPC has HTTPS reads, not a public WebSocket.
- V2 reserve-sync, token validation, cycle traversal, and directional dedup can be adapted; V3 local one-tick quotes **cannot** be used for real net-profit calculations.
- `src/arbitrage/preflight.rs` is a V2-only planner, not a signer, contract deployment, or Base-certified executor. Morpho/Aave contracts from Ethereum **must not** be assumed present/compatible on Base.
- The production Render worker auto-deploys `main`; keep this branch separate. No new Render/Replit service unless economic viability is observed.

## First bounded RPC check (no spending)
`node research/base/probe_rpc.mjs` makes at most **four** sequential HTTPS JSON-RPC reads, with timeout, chain ID check, no retries; response logs exclude endpoint, credentials and wallet information. Default official public endpoint is for low-volume research only.
- Must identify chain ID **8453 / 0x2105**.
- Record current block, gas price and fee history availability (not executable transaction costs).
- If no connectivity or rate limit, mark **UNVERIFIED**; do not claim the probe passed.
- Official RPC is rate-limited, and from Oct 8 2026 does not expose `debug`/`trace`/`txpool` namespaces. Historical state/transaction-index replay must be separately capability-tested on a permitted RPC.

## Economic experiment: hard stop/go
1. Identify one anchored token pair and two distinct *verified* Base pools/venues and exact swap fees, addresses, token ordering and real liquidity. Do not copy Ethereum addresses. Reject unsupported token behaviors.
2. Test one family of two-hop cycles with exact venue-specific quotes. Sample independent events/blocks; avoid reusing end-of-block prices as if available immediately after a trade.
3. Cap initial investigation at 2,000 events, tracking excluded/denied coverage and distinguishing gross, premium/funding, gas L2 + L1 data fee, pricing/slippage, and execution competition.
4. GO for further *simulation only* if at least **20 independently reproducible** events have conservative net proceeds > **2× all-in execution gas**; this threshold is a screening policy, not a forecast or proof of capturability.
5. NO-GO otherwise. Do not buy RPC plans or deploy a signer simply to manufacture more attempts. If gate passes, independently assess timing, failed-attempt costs, loss budget, exact calldata and contract safety before a funded canary.

**Status on creation:** not tested on Base; zero Base RPC observations, zero verified positive Base opportunities, zero funds authorized.
