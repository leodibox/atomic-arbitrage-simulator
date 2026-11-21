# Economic pivot: Base Aerodrome vs Uniswap V3

## Why switch markets

Ethereum mainnet Uniswap/Sushi versus Uniswap V3 owner-run one-shot
snapshots gave **80 valid quotes, 0 positive before gas** across
WETH/USDC/DAI/WBTC. New execution code for that exact strategy is
not justified. The existing worker and mainnet Guarded contract remain
unchanged.

Base (chain 8453) generally charges much less for execution than L1.
Its important venues include **Aerodrome** and **Uniswap V3**,
rather than the two V2 pools already researched. This uses verified
canonical Base factory, WETH, USDC, QuoterV2 and Chainlink addresses,
and the *pool's own actual fee-aware getAmountOut* (not assumed 30bps).

## Single read-only onchain decision, not a backtest

```powershell
cd "$HOME\Downloads\atomic-arbitrage-simulator"
git fetch origin
git switch research/base-aerodrome-v3-one-shot-20261120
git pull --ff-only origin research/base-aerodrome-v3-one-shot-20261120
node --check tools/base-aerodrome-uniswap-one-shot.mjs
$env:PATH = "$HOME\Downloads\atomic-arbitrage-simulator\tools\foundry;$env:PATH"
$env:BASE_RPC_URL = "https://mainnet.base.org"
node tools/base-aerodrome-uniswap-one-shot.mjs
```

The program uses `cast sig` to derive selectors (no handcrafted
Keccak library); Node built-in fetch for reads (no RPC subscription).
It reads **one same-block snapshot**, both Aerodrome stable/volatile
pools, and V3 500/3000 fee tiers in both directions, four sizes
0.05/0.10/0.25/0.50 WETH: at most 32 candidate routes,
with fewer if a pool is missing or below 5 WETH depth.

Flash fee is currently a **5 bps assumption from previous L1
evidence** and is NOT verified against Base Aave. The gas model
reserves 1m * 2 * eth_gasPrice for L2 execution plus a provisional
$0.50 L1 calldata buffer. **That is not exact transaction gas and
not a reliable guarantee**: total transaction bytes, L1 data cost,
inclusion fees, actual Base flash loan premium, and exact atomic
execution must be independently verified after a promising signal.
The script never reports an executable trade.

The existing Ethereum $5 sender hard cap must not be mistaken for
an authorized Base sender. There is **no Base sender**, bridge,
deploy, keys or trade in this PR.

Decision:
- no positive after assumed flash fee: stop this venue pair.
- positive but none after reserved costs: stop.
- positive indicative margin: evaluate **only the best candidate**
  in exact Base same-block multi-protocol fork state, including actual
  gas and Base flash premium; do not deploy yet.
- errors/missing quote: inconclusive; do not label as unprofitable.

Do NOT merge the experiment into Render's L1 mainline worker.
Only invest in Base smart contracts after finding an economically
interesting and rigorously verifiable opportunity.
