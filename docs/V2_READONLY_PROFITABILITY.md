# Read-only V2 profitability assessment — independent from Aave PR #10

This is a **research tool**, not a trade signal. It **never** signs, sends, deploys, approves, modifies Render settings, or alters the production scanner. The new script and unit tests live under `scripts/` only.

## Scope and verified contracts

The script queries Ethereum mainnet chain 1 at **one selected block** and checks:
- Uniswap V2 USDC/WETH pair: `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc`; factory `0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f`.
- SushiSwap V2 USDC/WETH pair: `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0`; factory `0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac`.
- Aave V3 Ethereum Pool: `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2`, queried for actual flash-loan premium.
- Chainlink Ethereum ETH/USD and USDC/USD feeds (read with `latestRoundData()` and `decimals()`), using block-time staleness gates. The latter avoids silently assuming USDC always equals exactly $1.

For each direction (Uniswap→Sushi and Sushi→Uniswap) the tool computes 100, 1,000, 5,000 and 10,000 USDC roundtrips with canonical V2 integer math. Premium is conservatively rounded up. It converts a **hypothetical** ETH gas cost into **USDC raw units** from the two Chainlink USD feeds, plus 20% buffer. All trades are marked `approvedForExecution=false` regardless of theoretical profitability.

## Usage

Node 20+; no dependencies. Put your Ethereum mainnet HTTPS JSON-RPC URL in a **private** environment variable. Do not paste an Alchemy key into chat, GitHub, code or logs.

```sh
node --test scripts/assess_v2_costs.test.mjs
HTTPS_URL="your private Ethereum Mainnet RPC URL" node scripts/assess_v2_costs.mjs
```

The second command should be run in a trusted private terminal or Codex environment with an appropriate secret binding. If no URL is configured it **fails closed**.

## Critical limitations

- The `400,000` gas units and `1 gwei` tip are **research assumptions**, not actual `eth_estimateGas` for profitable calldata. `block.baseFeePerGas` is current only for the sampled block; actual EIP-1559 inclusion cost can differ.
- Quoted reserves and Chainlink prices are pinned to a single sampled block, but quotes are **not executed** through the guarded Aave executor. They may change or be front-run before inclusion. Reading at a block is not enough to certify executability or positive realized P&L.
- This first tool supports **USDC/WETH V2 canonical 30 bps pairs only**, not V3 tick-crossing, V4 hooks or dynamically priced AMMs. Do not add unverified pair addresses to bypass fee/factory gates.
- It does not verify max loan liquidity, blacklist/pause conditions, signer, owner, executor bytecode, token transfer quirks, private relay, nonce fencing, MEV avoidance, wallet balance, max fee cap or safe daily loss budget.
- It is **not** integrated into a sender or the production scanner. Never treat a positive result as authorization to submit.
- Historical mainnet fork PR #10 independently showed both routes negative before gas at block `26436156`; this tool can re-evaluate with a fresh *read-only* block, but only once an RPC can be called.

## Acceptance criteria for Codex

Run Node tests with zero failures, run with a functioning mainnet HTTPS RPC, record exact sampled block, fresh feed timestamps, premium and all eight route outputs. Fail on stale feed/incorrect factory. Compare canonical integer outputs with the existing fork test on the **same block**. Make no production changes. Keep this research PR separate from the guarded Aave executor PR #10.
