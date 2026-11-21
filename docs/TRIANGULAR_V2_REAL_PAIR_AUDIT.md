# Read-only real-pair triangular V2 research

## Why this is necessary

Existing `src/arbitrage/cycles.rs` can find closed 2–4-hop
routes, and `src/arbitrage/net.rs` caps executable preflight at 3 hops.
But canonical `discover_weth_v2()` only adds WETH/token spokes,
so a pure WETH-star graph cannot form a three-token triangle
WETH→A→B→WETH. This is a discovery **coverage gap**, not evidence
of an existing profitable opportunity. The contract also needs three
distinct approved, standard 30bps V2 pairs; no token is enabled here.

## Test-only independent audit

`contracts/test/TriangularV2RealPairsFork.t.sol` performs
on-fork read-only evaluation for four existing, non-USDT markets:
USDC, DAI, WBTC and LINK. For six unique intermediate-token
combinations, it independently discovers 12 non-WETH cross-token
pairs across real Uniswap V2 / SushiSwap V2 factory registries,
when present. It checks real factory, pair code, both token
identities and actual reserves at the same Ethereum block.

For each combination, it evaluates both circular orientations
(WETH→A→B→WETH and reverse), and all 2x2x2 choices of
three pool factories, subject to distinct pairs. The two WETH
edge pools must each hold >=25 WETH and every pool must have
nonzero reserves; this does **not** certify depth or safety
of a middle non-WETH pool. Quotes use 997/1000 V2 fees for
every hop and scan ten loan sizes from the existing 0.25 WETH cap.

The conservative net estimate subtracts actual Aave premium,
a 10 bps output haircut and 1 million gas units
at `maxFeePerGas=2*baseFee+2 gwei`. The Chainlink ETH/USD quote
is checked for age <=3600sec; comparison of $5/$10/$15/$20
uses exactly the original one-shot USD gas buffer +10% and
round-up. **Only the $5 cap has any authority in the actual
sender**, which remains unchanged.

No `deal`, mocked liquidity, traces, fork transaction broadcast,
contract change, production allowlist change, USDT enablement,
operator authorization, private keys, wallet funds, or Render
deploy is involved.

### Windows PowerShell 5.1 — read-only fork test

```powershell
cd "$HOME\Downloads\atomic-arbitrage-simulator"
git fetch origin
git switch research/triangular-v2-cross-pairs-20261120
git pull --ff-only origin research/triangular-v2-cross-pairs-20261120
cd contracts
$env:PATH = "$HOME\Downloads\atomic-arbitrage-simulator\tools\foundry;$env:PATH"
$env:HTTPS_URL = "https://ethereum-rpc.publicnode.com"
Remove-Item Env:MAINNET_FORK_BLOCK -ErrorAction SilentlyContinue
forge test --match-contract TriangularV2RealPairsForkTest -vvv
if ($LASTEXITCODE -ne 0) { throw "Triangle fork audit failed or did not compile" }
```

The new suite was **OWNER-RUN: 1 PASS, 0 FAIL, 0 SKIP** on Ethereum mainnet fork block **26460047**. Observe the exact returned
`fork_block`, `cross_token_pairs_found_across_factories`,
`eligible_three_hop_route_variants`, `gross_positive_probes`,
`net_positive_probes`, `worst_case_gas_usd_e8` and $5 tier
count. Only eligible configurations are included; a missing
pair or insufficient liquidity must not be classified
as an arbitrage failure or profit.

### Decision criteria

Even if a hypothetical positive probe appears, do not execute
or modify the runtime. It must pass exact fork
`executeArbitrage()` from Guarded with the real pools at
the identical block, actual token transfer mechanics, complete
Aave repayment and post-gas / price impact / slippage checks,
then separate on-chain-bytecode/owner/nonce verification and
the single-attempt $5 sender restriction. Profitable
backtests never guarantee executable mainnet profit.

Keep this research branch **DRAFT** until full Foundry results
are collected. No contract or scanner code changes.

## Owner verified results — fork block 26460047

Foundry Solc 0.8.24 compiled successfully after the stack-depth
refactoring at commit `4170095`. The owner ran:
`forge test --match-contract TriangularV2RealPairsForkTest -vvv`.

- **1 test PASS, 0 FAIL, 0 SKIP**
- **10** factory-verified cross-token pairs discovered out of 12 possibilities.
- **60** eligible three-hop route variants including both directions and
  Uniswap/Sushi pool combinations.
- **600** loan-size probes using real observed V2 reserves.
- **0** gross-positive probes and **0** positive-net probes.
- **0** candidates within each hypothetical $5 / $10 / $15 / $20 gas tier.
- Max modeled fee with safety margin: `396721109` USD-e8 =
  **$3.96721109**.
- Max modeled ETH gas: `2226172652000000 wei` =
  **0.002226172652 ETH**.
- This modeled gas estimate is within the existing $5 gate in the fork block.

Data are a **single block snapshot**, not a time series. No profitable
swap was observed in these candidate routes. The test does not prove the
absence of profits in other tokens, sizes, pools, venues, blocks or
strategies, and does not perform exact Guarded contract execution
simulation for real triangular routes. No signer or mainnet action.
Only audit tests and documentation were changed; no runtime discovery,
execution controls, production contract or gas cap was modified.

Decision: acceptable to archive/merge the audit evidence only.
Next economically meaningful investigation is to compare
alternative venues / V3 read-only coverage or a bounded historical
block sample before spending deployment gas. Neither is authorized
by these tests to execute live operations.
