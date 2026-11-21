# USDT positive-path + additional V2 coverage + $5 gas audit

Status: **READY FOR LOCAL FOUNDRY EXECUTION; RESULTS NOT YET CERTIFIED**.
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
