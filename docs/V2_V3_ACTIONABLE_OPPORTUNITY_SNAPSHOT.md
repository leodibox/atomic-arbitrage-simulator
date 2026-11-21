# One decision, real quotes: V2 vs V3 mainnet arbitrage

This is a **single, bounded opportunity scan**, not a new suite of
synthetic tests and not an execution system.

Run on Windows PowerShell (Node 18+):

```powershell
cd "$HOME\Downloads\atomic-arbitrage-simulator"
git fetch origin
git switch research/actionable-v2-v3-quotes-20261120
git pull --ff-only origin research/actionable-v2-v3-quotes-20261120
$env:HTTPS_URL = "https://ethereum-rpc.publicnode.com"
node tools/v2-v3-opportunity.mjs
```

What it actually does:
- Pins all RPC reads and Uniswap V3 QuoterV2 calls to a **single recent mainnet block**.
- Verifies both canonical Uniswap/Sushi V2 WETH/USDC pool identities,
  actual factories and onchain reserves.
- Calls the **real QuoterV2** (not a single-tick approximation)
  for both WETH→USDC and USDC→WETH on Uniswap V3 fee tiers
  500 (0.05%) and 3000 (0.3%).
- Compares 0.01/0.05/0.1/0.25 WETH, both route orientations,
  both V2 pools, 2 V3 fee tiers = **32 bounded cases**.
- After each quote subtracts a conservative 5-bps flash premium
  (previously verified in fork, not refreshed in this script),
  10-bps output haircut, 1,000,000 gas units using EIP-1559
  2×baseFee+2gwei; verifies the gas ceiling converted through
  fresh Chainlink ETH/USD with +10% margin is below $5.
- Logs exact failure counts. Fails closed if a quote errors or
  oracle is stale; never reports unavailable samples as no profit.
- Uses built-in Node fetch only: no SDK, dependencies, signer,
  wallet access, gas spend, deployment, or Render config.

**Decision rule:** if `actionableForNextFork=0`, do not build
a V3 executor or spend on deployment based on this opportunity set.
If >0, first examine the exact candidate with **real onchain pool
and QuoterV2 execution** at the same block and verify full
profitability/safety; merely a quote is NOT an execution approval.
A live V3 trade requires different executor path: current
Guarded contract is V2-only.

The purpose is to end repeated code-only fork tests and get one
falsifiable economic signal. This script is a **source-only
implementation until the Windows run output arrives**.

Important: the existing Rust live scanner, allowed tokens, legacy
USDT exclusion, mainnet hard $5 one-shot gate, operator, contract,
Render worker and wallet remain unchanged.
