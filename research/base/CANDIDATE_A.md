# Candidate A — Base WETH/USDC cross-venue V3 (read-only screening)

**Status: OFF-CHAIN ADDRESSES IDENTIFIED, ON-CHAIN STATE NOT YET VERIFIED.**
No opportunity has been confirmed, no RPC method was executed successfully against Base from this chat environment, and there is no execution approval.

| Venue | Address | Advertised fee | Reference |
|---|---|---|---|
| Uniswap V3 | `0xd0b53d9277642d899df5c87a3966a349a798f224` | 0.05% (500 V3 fee units) | https://www.geckoterminal.com/base/pools/0xd0b53d9277642d899df5c87a3966a349a798f224 |
| SushiSwap V3 | `0x57713f7716e0b0f65ec116912f834e49805480d2` | 0.05% (500 V3 fee units) | https://www.geckoterminal.com/base/pools/0x57713f7716e0b0f65ec116912f834e49805480d2 |

Base WETH `0x4200000000000000000000000000000000000006` is documented by Uniswap. Base native USDC is `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913`. Do not copy Ethereum Mainnet WETH or factory addresses.

## Reproducible bounded check (Node 22+)

```sh
node --test research/base/verify_pools.test.mjs
node research/base/probe_rpc.mjs
node research/base/verify_pools.mjs
```

These **commands have been prepared, not executed successfully against Base** here. The verifier performs 16 sequential reads at most: `eth_chainId`, `eth_blockNumber`, and for each pool `eth_getCode` plus six `eth_call` reads (`token0`, `token1`, `fee`, `liquidity`, `slot0`, `factory`). Checks reject wrong chain, missing code, wrong token pair/fee, zero liquidity and malformed RPC responses. No retries, private keys, call simulation with trading capital, or transaction-sending RPC methods.

The factory addresses are OBSERVED ONLY, not yet certified by `getPool` and canonical DEX factory identity. Further stage must verify each factory mapping and on-chain historical state, tick crossings, fee growth and active liquidity against an exact canonical Quoter/contract calculation. A simple square-root price derived from slot0 is NOT an executable amountOut.

## Economics

The two advertised 0.05% pools imply **10 bps of swap fees** before price impact, any flash borrowing fee, and L2 execution + L1 data gas fees. A potential cross-pool quote must overcome that entire hurdle. Neither quoted pool page establishes a cross-venue arbitrage profit.

Proceed to the 2,000-event limit only after both pools are validated and exact quotes can be independently reconstructed. GO requires at least 20 independently reproducible events with conservative net >2× all-in gas AND evidence of timeliness. Otherwise NO-GO. Compare costs in the same currency without assuming gas is free. No new paid RPC or hosting without an explicit spending decision.
