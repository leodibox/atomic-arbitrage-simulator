# Alchemy setup — Ethereum atomic arbitrage simulator

This project uses Rust/Alloy and connects to Ethereum Mainnet using **HTTPS_URL** and **WSS_URL**. Its existing code does **not** read an `RPC_HTTP` or standalone `ALCHEMY_API_KEY` environment variable for provider setup. Choose Alchemy's standard API-key path; **do not enable wallet-based x402/MPP payments** for the free-tier simulator.

## 1. Install Alchemy Agent Skills for your coding agent

The official command, run from the repository root on your own machine/Codex workspace, is:

```powershell
npx skills add alchemyplatform/skills --yes
```

For convenience, this repository also vendors the official Alchemy `alchemy-api` skill entry point at `.agents/skills/alchemy-api/SKILL.md` (upstream: https://github.com/alchemyplatform/skills/blob/main/skills/alchemy-api/SKILL.md ). The official command installs the **full bundle and its reference files**; this checked-in entry point is a standalone quickstart and does not claim to replace the entire bundle.

## 2. Create/select an Alchemy app (free)

1. Sign in at https://dashboard.alchemy.com/
2. Create an app named **Atomic Arbitrage Simulator** if one does not already exist.
3. Set the chain/network to **Ethereum Mainnet**.
4. Copy the app's **HTTPS** and **WSS** endpoint URLs via the dashboard. Do not post them in chats, issues or commits; they include your API key.

## 3. Configure the simulator

In your local, git-ignored `.env`, or **Render > Service > Environment**, set the following runtime variables:

```text
HTTPS_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_API_KEY
WSS_URL=wss://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_API_KEY
CHAIN_ID=1
POOL_SCAN_ENABLED=false
POOL_ALLOWLIST=
DEBUG=true
```

Use the complete URLs from the dashboard, not the literal placeholder. Keep the remaining required settings from `.env.example`, such as `BOT_ADDRESS`, `PROFIT_RECIPIENT_ADDRESS`, `MIN_REVENUE_WEI`, `BRIBE_BPS`, `MAX_GAS_PRICE_GWEI`, and `WETH_FLOAT`. The code requires a nonzero valid `PROFIT_RECIPIENT_ADDRESS` and a parsable `BOT_ADDRESS`.

For **simulation-only**, leave `PRIVATE_KEY` and `IDENTITY_KEY` unset. Do not fund a wallet or enable transaction signing as part of RPC setup.

**Note:** `POOL_SCAN_ENABLED=false` and an empty `POOL_ALLOWLIST` are deliberate safeguards: the worker stays idle rather than scanning all pools. Once you have a *verified* allowlist, configure it explicitly and test in simulation mode. The project currently does not broadcast transactions in this path.

## 4. Validate RPC without exposing keys

Run this in **PowerShell**, replacing the placeholder with the actual HTTPS URL **locally** (do not paste its output if it contains secrets):

```powershell
$rpc = "https://eth-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_API_KEY"
$body = '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}'
(Invoke-RestMethod -Uri $rpc -Method Post -ContentType "application/json" -Body $body).result
```

Expected result for Ethereum Mainnet is **`0x1`**. This checks the HTTPS endpoint only; verify WebSocket connectivity separately before enabling any subscribing worker. An actual authenticated RPC smoke test remains pending until the Alchemy account/app and endpoint are configured.

## Security and scope

- Keep `.env` out of GitHub (already ignored by `.gitignore`).
- Put complete URLs in Render's secret environment variables, not in tracked source code or GitHub issues.
- Alchemy is a blockchain RPC/data provider; installing agent skills does **not** provision an account or deploy this application.
- No changes to trading logic, wallets, transactions, or deployment are required to add the skill.
