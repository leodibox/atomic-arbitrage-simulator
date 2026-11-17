# WETH/USDT and WETH/DAI research — never executes

This work is separate from PR #10 (guarded Aave executor), PR #11 (USDC/WETH economics) and the production Rust scanner. It is **read-only** and never produces an executable approval.

## Scope

At a single pinned Ethereum Mainnet block (default `finalized`), for each of **WETH/USDT** and **WETH/DAI**, the Node script:

- Queries canonical Uniswap V2 and SushiSwap V2 factories with `getPair(WETH, token)`. Never guesses pool addresses; a missing pool becomes `UNAVAILABLE`.
- Verifies chain 1, token bytecode and actual decimals (USDT 6, DAI 18, WETH 18), pair bytecode, `factory()`, `token0()`, `token1()`, and `getReserves()`. Uses actual token ordering for reserves.
- Reads canonical Aave V3 `FLASHLOAN_PREMIUM_TOTAL()` and EIP-1559 block base fee.
- Estimates both WETH→token→WETH directions for 0.01, 0.05, 0.1, 0.5 and 1 WETH per market (up to 20 route observations, if both pairs exist).
- Uses the integer 997/1000 V2 swap formula for both hops, subtracts rounded-up Aave premium and **hypothetical** 400k gas × (block base fee + 1 gwei tip) × 1.2 safety buffer. Because the borrowed asset is canonical WETH, costs already share the correct 18-decimal ETH/WETH unit and require **no ETH-to-USDC oracle conversion**.
- Reports minimum WETH reserve across the two venues to identify thin liquidity. Output includes pinned block number/hash and input and output raw wei.

## Running safely

Node 20+:

```sh
node --test scripts/assess_v2_weth_pairs.test.mjs

# HTTPS_URL must be privately set, never included in chat, GitHub or commit logs:
node scripts/assess_v2_weth_pairs.mjs
```

Optional `V2_BLOCK_TAG=finalized`, `safe`, `latest`, or `0x...` fixed block when supported by RPC. No key appears in output. On a Codex workspace, if the RPC is not injected privately, a temporary reputable public read-only endpoint may be used after confirming chain 1; never publish credentials.

## Limitations

This is **not a verified opportunity or live transaction**. The default gas units and tip are arbitrary research inputs, not `eth_estimateGas`; it does not produce exact `eth_call` success, quote V3/V4, verify Aave WETH liquidity or token transfer properties, simulate mempool/inclusion or quantify revert gas. DAI/USDT routes might not exist on both factories or might be very thin. Any future profitable snapshot can disappear before inclusion and does **not** authorize trading.

`approvedForExecution=false`, `transactionSubmissionEnabled=false`, `gasEstimateVerified=false` and `executableCalldataVerified=false` are mandatory hardcoded output fields. No signer, owner permissions, auto-payout, wallet funds, Render configuration, smart contract deployment or code paths in production are modified.

## Acceptance conditions

1. Run the Node unit suite with 0 failures. The development prototype passed 10/10 Node tests locally; validate the committed files independently in Codex.
2. Sample `finalized` (preferred) on mainnet with exact current pair identities and recorded reserves, decimals, block hash and Aave premium.
3. Publish all results, including unavailable pairs and losing routes. Report value in WETH (wei/1e18), not mislabeled USDC.
4. Compare findings to PR #11 as a separate market; keep both PRs in DRAFT.
5. Do not merge or deploy from a positive discovery. Require execution-level review and real post-gas conditions first.
