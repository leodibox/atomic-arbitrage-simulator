# USDT positive-path + additional V2 coverage + $5 gas audit

Status: **OWNER-RUN FOUNDRY FORK CERTIFIED: 3 PASS, 0 FAIL, 0 SKIP (2026-11-20)**.
This branch adds **two fork test files plus this documentation only**.
No changes to Solidity production contract, Rust scanner, allowlists,
USDT exclusion, transaction sender, wallet, database, Render or $5 cap.

## Phase A — successful real-token/Aave integration with synthetic AMM

File: `contracts/test/GuardedPositiveUsdtFork.t.sol`

Two tests on real Ethereum mainnet fork:
1. **10,000 real USDT** borrowed from the actual Aave V3 mainnet pool.
   Executes USDT→WETH→USDT over locally deployed, **synthetic** V2
   pairs with deliberately unequal reserves; actual WETH and USDT token
   contracts are used for both transfers. Validates principal and
   premium repaid to the real Aave aToken, allowance fully consumed,
   positive net token balance in Guarded, and no unapproved recipient
   payment. This hits Guarded's own real-USDT transfer/approval path.
2. **0.1 real WETH** borrowed via actual Aave, WETH→USDT→WETH over the
   same fork-only synthetic pairs. Checks return-data-less USDT output
   transfer handled end-to-end in Guarded with real contracts.

Explicit synthetic reserves: 500 WETH / 1m USDT and
250 WETH / 1m USDT. They are created using `deal` and `sync` in a
locally deployed V2-style fixture enforcing the 30 bps invariant.
**These reserves are not current Uniswap/Sushi prices** and their
positive outputs prove technical integration only, never market profit.
Do not add test fixture pools to any allowlist.

## Phase B — five additional real Ethereum V2 token markets

File: `contracts/test/ExtendedV2CoverageAuditFork.t.sol`

Audit AAVE, LDO, CRV, SHIB, PEPE against the existing Ethereum
Uniswap V2 and SushiSwap V2 factories:
- Retrieve actual `factory.getPair(WETH, token)`.
- Require code, onchain `pair.factory()`, token0/token1 identity.
- Fetch actual reserves, require both nonzero, **>=25 WETH**.
- Only if **both factories** have an eligible pair for the same token,
  compare two directions across four WETH loans:
  `0.01, 0.05, 0.1, 0.25 WETH`.
- Log all accepted/filtered pools, and counts of positive-gross and
  positive-net hypothetical directions. An absent or under-liquid
  pair is observed but **not** automatically authorized. This test does
  not claim all five markets are eligible, profitable, executable, or
  safe for the legacy executor.

## Phase C — same conservative $5 fee budget

For exactly the same fork block, use:
- Aave `FLASHLOAN_PREMIUM_TOTAL()` at the fork block;
- constant product Uniswap/Sushi 30 bps fee per hop;
- 10 bps haircut on final WETH output;
- 1,000,000 gas units, EIP-1559 maxFeePerGas = 2*baseFee + 2 gwei;
- Chainlink mainnet ETH/USD (8 decimals), checked round and freshness
  <=3600 seconds (diagnostic freshness only, NOT execution validity);
- USD ceiling = `ceil(gasWei * ethUsdE8 * 110 / (1e18 * 100))`;
- result tier counts under $5, $10, $15 and $20.

USD tiers never alter onchain gas or net profit. The real sender's
hard `MAX_TOTAL_GAS_USD_E8 = 500000000` **remains unchanged**.
Positive net estimates are **not** end-to-end contract simulations.
Even a positive net estimate can fail on token behavior, market changes,
MEV competition, oracle freshness, gas limit, Aave capacity or nonce.

## PowerShell 5.1 — free, local fork-only commands

```powershell
cd "$HOME\Downloads\atomic-arbitrage-simulator"
git fetch origin
git switch research/usdt-positive-v2-coverage-20261120
git pull --ff-only origin research/usdt-positive-v2-coverage-20261120
cd contracts
$env:PATH = "$HOME\Downloads\atomic-arbitrage-simulator\tools\foundry;$env:PATH"
$env:HTTPS_URL = "https://ethereum-rpc.publicnode.com"
Remove-Item Env:MAINNET_FORK_BLOCK -ErrorAction SilentlyContinue
forge test --match-contract GuardedPositiveUsdtForkTest -vvv
if ($LASTEXITCODE -ne 0) { throw "USDT positive fork suite failed" }
forge test --match-contract ExtendedV2CoverageAuditForkTest -vvv
if ($LASTEXITCODE -ne 0) { throw "V2 market and budget audit failed" }
```

**Before interpreting output:** both suites must compile and run with
`0 skipped`; if RPC prunes the requested block, use unpinned latest
as above. Do not use `--broadcast`, deploy, send funds, paste wallet
private keys, or edit Render variables.

Decision: we may consider widening **read-only discovery** only
after confirming verified pair identities, enough fresh liquidity
and estimated net margin, and separately reviewing token transfer
compatibility. No profitable path implies no USDT authorization or
runtime configuration change.

## Owner-run fork results, November 20, 2026

Owner used Foundry 1.9.0 + Solidity 0.8.24 against
`https://ethereum-rpc.publicnode.com` with no fixed historical block.
Both suites compiled and passed, **0 failures / 0 skipped**.

**Positive-path real asset and Aave integration, fork block 26459944**
- Real USDT loan: 10,000 USDT, premium 5 USDT, synthetic V2 reserves
  produced **9,298.953512 USDT** retained inside Guarded after
  successful real Aave repayment; 416161 locally measured gas units.
- Real WETH loan: 0.1 WETH, premium 0.00005 WETH, synthetic V2 reserves
  produced **0.098593599043823469 WETH** retained after repayment;
  402285 locally measured gas units.
- Tests asserted actual Aave aToken receives the premium, full repayment,
  no residual Aave allowance, recipient balance unchanged, profit retained.
- The huge positive figures are **fabricated by the synthetic AMM
  reserves, NOT observed market profit**, cannot be used as live routes
  and are not net of gas.

**Additional real V2 market read-only coverage, fork block 26459945**
All 5 tokens checked against real Ethereum Uniswap V2 and SushiSwap V2
factory, pair, token ordering, and reserves.

| Token | Uniswap WETH | SushiSwap WETH | both >=25 WETH? |
|---|---:|---:|---|
| AAVE | 16.439081280199098417 | 3.062680973842038898 | No |
| LDO | 2.459116709174439238 | 6.062193477141781914 | No |
| CRV | 20.018318761051798548 | 3.083758306438470128 | No |
| SHIB | 113.036287228653660182 | 0.437328650525245480 | No |
| PEPE | 6908.986217645945553077 | no Sushi pair | No |

9 real pairs discovered; 0 token markets have two pools above 25 WETH.
0 gross-positive comparisons and 0 net-positive comparisons **because
no markets met the pair-pair liquidity eligibility requirement**.
Thus zero opportunities in this experiment are *not evidence* that
higher budget prevents success or that no other route exists.

**Conservative cost comparison at same fork block**
- Chainlink ETH/USD quote age: 2136 seconds (diagnostic-only).
- Conservative maximum gas in ETH: `2230420120000000 wei` (~0.00223042012 ETH).
- Conservative USD gas ceiling with 10% buffer:
  `397478040` USD-e8 = **$3.97478040**.
- Within real sender hard $5 cap: true.
- Positive-net candidates under each $5, $10, $15, $20 ceiling:
  **0 / 0 / 0 / 0**.
- Gas estimate uses 1 million units. Reported test `gasleft()`
  consumption is a separate local measurement, not actual transaction
  spend or proof of an exact production gas limit.

**Certification decision**
USDT integration is technically viable with the Guarded contract
on fork using synthetic pairs and real asset/Aave token interactions,
but mainnet profitability and exact sender simulation remain
unproven. No production scanner inclusion yet. All code in this PR
is tests and documentation only; preserve original hard $5 gate,
USDT execution restriction, owner/operator controls, and disable
all mainnet deployment and trading.
