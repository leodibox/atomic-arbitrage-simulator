# atomic-arbitrage-simulator
Ethereum atomic arbitrage simulator using Rust, Alloy, REVM, Foundry, and Aave V3 flash loans. Simulation-first, with profitability checks and secure execution.

## Read-only V2 net screening on Render

After PR #21 is independently verified, keep the existing worker and its current
`WSS_URL` (Ethereum mainnet), `HTTPS_URL`, and non-empty `POOL_ALLOWLIST`.
Set `POOL_SCAN_ENABLED=false`, `CHAIN_ID=1`, and `SIM_V2_GAS_UNITS=1000000`
in the worker environment to opt in. The example is a conservative screening
budget, not a measured transaction estimate. Use a higher budget if local executor
simulations, with headroom, exceed it. Keep the rest of the worker configuration
as-is. No signer or new credential is required.

The screen budgets gas at twice the block base fee plus a 2 gwei priority fee,
reads the Aave premium at the reserve block, and discounts quoted output by
10 basis points. The estimates apply to WETH-denominated routes compatible with
the legacy V2 executor. USDT and V3 routes remain visible in gross discovery
and are excluded from net screening.

After an approved rollout, check for `canonical Ethereum WETH V2 discovery ready`
and `read-only WETH V2 net screen`. Read `verified_v2_pools` and
`positive_net_estimates` in the latter. A zero count means no qualifying route
was found in the sampled block. An availability warning means pricing was
unavailable for that scan. Remove `SIM_V2_GAS_UNITS` to return to gross discovery.
