# Base economic gate progress — 2026-11-19 UTC

## What was implemented (read-only)
- `uniswap_quote.mjs`: canonical Uniswap V3 factory `getPool(WETH, USDC, 500)` check on Base 8453, then six **single-venue** Uniswap QuoterV2 `quoteExactInputSingle` calls at one pinned block. Fixed input sizes: USDC 100/500/1,000/2,000 and WETH 0.05/0.1. It logs raw output, initialized ticks crossed and Quoter gas estimate. Bounded to ≤12 RPC calls, no retries, no private keys and no signing/submission.
- `uniswap_quote.test.mjs`: mocked regression tests cover ABI selector/encoding, wrong chain, incorrect canonical pool and pinned-block/no-send behavior. A corresponding locally executed prototype passed **4/4** Node tests; the exact GitHub branch version has not been executed by CI or against live Base from this session.
- Official Uniswap Base deployment reference: https://developers.uniswap.org/docs/protocols/v3/deployments/v3-base-deployments
- Uniswap QuoterV2 ABI: https://github.com/Uniswap/swap-router-contracts/blob/main/contracts/interfaces/IQuoterV2.sol

## Market selection
- Candidate A: Uniswap V3 0.05% vs SushiSwap V3 0.05% WETH/USDC. Sushi TVL shown by third-party provider ~USD 47k. Combined nominal swap fee = 10bp before impact/funding/gas.
- Sushi 0.01% pool `0x482fe995c4a52bc79271ab29a53591363ee30a89` third-party TVL ~USD 2.7k, very thin.
- Aerodrome SlipStream 0.01% pool `0xdbc6998296caa1652a810dc8d3baf4a8294330f1` third-party TVL ~USD 92k. It is a promising **research alternative**, NOT the same Uniswap V3 execution ABI/quoter; needs separate factory and tick/math certification. Source: https://www.geckoterminal.com/base/pools/0xdbc6998296caa1652a810dc8d3baf4a8294330f1

## Critical STOP
- Direct public Base RPC from this analysis container failed DNS lookup; **zero live read results**.
- The Uniswap Quoter gas estimate is NOT all-in L2+L1 data fees or actual cross-venue execution gas.
- Only one venue is quotable by this script; NO same-block executable two-venue roundtrip quote, historical 2,000-event replay, or verified P&L exists. Do not mark GO.
- Keep draft PR #19 separate from auto-deploying `main`. No paid plan, new hosting, contracts, keys or funds authorized.

## Next acceptance step
On a network-enabled authorized runner, execute `node --test research/base/*.test.mjs`, then `node research/base/probe_rpc.mjs`, `node research/base/verify_pools.mjs`, `node research/base/uniswap_quote.mjs`. Reject incorrect canonical factory mapping; independently obtain canonical Sushi/Aerodrome venue-specific exact quotes before computing two-hop net returns. Stop after bounded tests if an RPC provider denies access.
