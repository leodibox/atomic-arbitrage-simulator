# Morpho Blue + Uniswap V3: read-only feasibility, never trading

## Why this research exists

Uniswap V2 roundtrips researched in PRs #11 and #12 were negative even before gas. This new branch **does not add more V2 tokens** or alter the running scanner. It checks if a **0-premium flash funding candidate (Morpho Blue)** and **1 bp / 5 bp V3 pools** exist on Ethereum mainnet, so future work can focus on lowering fee barriers rather than expanding an unexecutable allowlist.

## Official protocol references

- Morpho Blue mainnet singleton `0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb`, from https://docs.morpho.org/developers/contracts/addresses/
- Morpho `flashLoan(token,assets,data)` documentation: https://docs.morpho.org/developers/contracts/blue/ documents `flashFee = 0`, with `maxFlashLoan` constrained by the token balance actually held by the Morpho singleton. It is **not** an Aave `flashLoanSimple` API and needs a separate authenticated callback.
- Uniswap V3 factory `0x1F98431c8aD98523631AE4a59f267346ea31F984`, official Ethereum deployment docs: https://developers.uniswap.org/docs/protocols/v3/deployments/v3-ethereum-deployments
- Uniswap V3 fees 100 (1 bp) and 500 (5 bp), official docs: https://support.uniswap.org/hc/en-us/articles/20904283758349-What-are-fee-tiers
- Official Uniswap V3 pool implements **different** `swap(...)` + `uniswapV3SwapCallback` and `flash(...)` + `uniswapV3FlashCallback`. These paths have different funding/fee semantics: https://github.com/Uniswap/v3-core/blob/main/contracts/UniswapV3Pool.sol

## What the script does (and does not do)

`scripts/research_morpho_v3_readonly.mjs` connects through a private `HTTPS_URL` or permitted temporary public Ethereum RPC, checks chain ID = 1, gets one finalized block and pins **all** code and state reads to that block number.

It verifies canonical Morpho, Aave, Uniswap V3 factory, WETH and USDC **contract existence**, then reads:
- `WETH.balanceOf(Morpho)` and `USDC.balanceOf(Morpho)` — current token cash, **not** an actual flashLoan success, spendable-by-this-bot proof or formal approval.
- `Aave Pool.FLASHLOAN_PREMIUM_TOTAL()` — actual premium in bps, at the sampled block.
- `UniswapV3Factory.getPool(USDC,WETH,fee)` for both 100 and 500, with `factory()`, `token0()`, `token1()`, `fee()`, `liquidity()` and bytecode verification. Missing pools report `POOL_MISSING`; zero active in-range liquidity is explicitly marked.

It prints fee-only scenarios for a **two-swap roundtrip** with a hypothetical V3/V2 or V3/V3 combination, comparing 0 bps Morpho **documented** versus on-chain Aave premium. For a V3 pool with 5 bp swap fee, a 5+30 bp two-swap route with Morpho has fee-only hurdle 35 bp; using Aave 5bp raises it to 40 bp. It does **not** claim any profitable routes: pool prices, full V3 tick crossings, actual swap execution, extra gas, transaction inclusion, ERC20 transfer anomalies, Morpho callbacks, Aave/Morpho actual liquidity availability and risk limits are not evaluated.

The comparison **must not** confuse a v3 `swap()` that temporarily sends output before callback payment with invoking a separate v3 `flash()` loan, which **charges its own fee**. Fee-only arithmetic is a lower bound, not an executable economics report.

Always outputs `approvedForExecution=false`, `netProfitVerified=false` and `transactionSubmissionEnabled=false`.

## Running in Codex

Node 20+:

```sh
node --test scripts/research_morpho_v3_readonly.test.mjs
node scripts/research_morpho_v3_readonly.mjs
```

Supply `HTTPS_URL` securely, not in source, PR comments, logs or chat. Use `V3_BLOCK_TAG=finalized` (default) or a specific block selector when the RPC supports historical state. In some Codex environments outbound traffic may require `NODE_USE_ENV_PROXY=1`; do not disable TLS.

No successful live RPC run is claimed until verified by Codex. Local and GitHub Actions test verification remains pending.

## Acceptance / next stages

1. Verify 8 Node tests, run on canonical Ethereum mainnet, report exact pinned block hash, Morpho WETH/USDC cash, Aave premium and V3 1/5 bp pool identity/liquidity. Mark all unavailable or uncertain conditions explicitly.
2. Write independent **Foundry fork tests** exercising actual Morpho `flashLoan` with separate callback and **real V3 swap** via `uniswapV3SwapCallback`, with no synthetic profitability assertion; compare expected fees and actual emitted/returned balances.
3. Adopt tick-crossing-exact QuoterV2 or validated local simulation, then bounded size search. Independently test 1bp/5bp token pairs against historical state, gas consumption and potential failed attempt costs.
4. Only after independent security review: consider a **separate** guarded V3 executor and robust sender. No merge, deployment, contract unpause, signer or money movement authorized by this PR.

## Cautions

Balancer V2 is **not** assumed 0 bps or available here. Its fee, pause status and vault cash require a separate verified on-chain read after the November 2025 exploit. A repo or paper cited by a third party is not treated as verified merely by name: arXiv `2601.19570` refers to a sandwich-attack study, not a proof of arbitrage concavity.

All fee savings are hypothetical until a coherent exact-call transaction shows positive **net** expected value after gas and revert/inclusion risk.
