# Read-only $5 / $10 / $15 / $20 fee counterfactual

## Question
Are we losing Ethereum V2 flashloan opportunities because the one-shot first
trade has a **maximum worst-case $5 USD gas cost**?

## What this PR adds
Every 25th observed block (same Chainlink cadence as before), log:
- `max_usd_e8`: the conservative estimated gas charge in USD * 1e8,
  calculated from the existing `gas_units`, EIP-1559 maximum fee
  `(2 * baseFee + configured tip)`, Chainlink ETH/USD and +10% buffer.
- `positive_after_estimated_costs`: unique routes passing the scanner's
  current conservative net and V2 preflight estimates.
- `routes_within_5_usd`, `routes_within_10_usd`,
  `routes_within_15_usd`, `routes_within_20_usd`: same candidate
  routes placed under four *counterfactual gas-spend ceilings*. All values
  are from the SAME block, same loan probe amounts, same gas estimate,
  same slippage and same market snapshots.
- `additional_routes_5_to_10_usd`,
  `additional_routes_10_to_15_usd`,
  `additional_routes_15_to_20_usd`: incremental hypothetical counts.

If the network fee estimate is $9, all profitable estimated candidates
count under $10/$15/$20 and zero under $5. **This does not claim that
there were actual missed trades.** In particular, `v2_gross_cycles = 0`
means no gross positive cycles were even found among the observed pools
and loan sizes, so raising the budget alone would not create opportunities.

## Strong limitations
- `find_cycles` only discovers cycles over eight currently eligible WETH
  V2 pairs and the existing amount probes. It does NOT cover all of
  Ethereum, all tokens, route types, private pools or pending blocks.
- This compares **budget ceilings**, not alternate gas settings. Increasing
  USD willingness does NOT automatically reduce gas costs or improve net
  output; the actual gas cost is the same in every hypothetical tier.
- A route is only a **read-only estimate**: no exact sender `eth_call`,
  independent profitable simulation, nonce, signer or durable one-shot
  ledger check here. External MEV competition can invalidate quotes.
- The oracle's periodic freshness allowance is observational only and
  **not sufficient for production signing**. If oracle is unavailable,
  the scanner prints `unavailable` instead of fabricating USD values.
- **No changes** to `one_shot::MAX_TOTAL_GAS_USD_E8` (500000000),
  to preflight or `NetScan` configuration, or to operator approval,
  contract deployment, wallet, gas spend, databases or broadcasts.
- With zero accepted net routes, all tiers report zero regardless of
  gas price; this does NOT prove that $5 will never miss opportunities.

## Validation

GitHub Actions cannot start due account billing lock. Before merging,
run on a local environment with Rust cargo toolchain:

```powershell
cd "$HOME\Downloads\atomic-arbitrage-simulator"
git fetch origin
git switch feature/read-only-usd-budget-tiers-20261120
git pull --ff-only origin feature/read-only-usd-budget-tiers-20261120
cargo test usd_tiers_are_monotone_and_never_change_the_sender_cap
cargo test
```

If `cargo` is not installed, this is still unverified; do not claim
passing tests or deploy this scanner modification without compilation.
The contract Foundry tests do not validate Rust scanner changes.
