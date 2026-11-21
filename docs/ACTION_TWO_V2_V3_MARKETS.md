# One action: two other markets, one decision

Following real WETH/USDC V2/V3 scan at block 26460257 (**32/32
quotes, zero positive before gas**), run a bounded alternative-market
snapshot on Ethereum mainnet without deploying or spending gas.

Markets: **WETH/DAI and WETH/WBTC**, Uniswap V2 and SushiSwap V2
versus Uniswap V3 0.05% / 0.3%, both directions at
0.05 / 0.10 / 0.25 WETH. Up to **48 actual same-block
QuoterV2 comparisons**. Fetches pair addresses from the V2
factories, verifies pair's on-chain factory and token addresses,
and only compares pools with >=25 WETH. Missing or too-small
pools appear separately in `discovered`.

Read-only conservative screen: previously observed Aave premium
5 bps (must reverify for exact transaction), output haircut 10bps,
1m gas units using 2*baseFee + 2gwei, Chainlink fresh ETH/USD
(max 1h), 10% gas margin; mainnet $5 sender cap remains unchanged.
No execution authorization, no token transfer, key, wallet or
transaction broadcaster. Positive quoted spreads require a
separate *exact* Guarded compatibility test, because the
current production Guarded contract is V2-only.

Run once (Windows PowerShell):
```powershell
cd "$HOME\Downloads\atomic-arbitrage-simulator"
git fetch origin
git switch research/action-two-markets-v2v3-20261120
git pull --ff-only origin research/action-two-markets-v2v3-20261120
node --check tools/v2-v3-two-markets.mjs
$env:HTTPS_URL = "https://ethereum-rpc.publicnode.com"
node tools/v2-v3-two-markets.mjs
```

**Stop rule:** If `actionableForNextFork` is zero AND no errors
occur, do not write a V3 executor for these markets. If >0,
replay only the best real candidate at the same block.
If errors >0, mark inconclusive, not a claim of no opportunities.
This is one snapshot, not a historical backtest or future profit
guarantee. Source is published but execution is not yet observed.
