# Guarded mainnet deployment preparation — owner + recipient

**Status:** prepared only. NOT DEPLOYED. This guide does not approve spending.
**Chain:** Ethereum mainnet, chain ID 1.
**Designated wallet (user-confirmed):** `0x49826Aa476d6388fcdccAD7b3661597986a53132`.
**Contract:** `contracts/src/FlashLoanArbGuarded.sol`, Solidity compiler **0.8.24**.
**Constructor arguments:**
- `aavePool`: `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2`
- `recipient`: `0x49826Aa476d6388fcdccAD7b3661597986a53132`

## Before spending ETH
1. Have the exact source and all relevant tests reviewed independently; specifically audit Aave callback, owner-only access, arbitrary approved pairs, 30bps swap assumptions, token transfer/approval edge cases and repayment/payout. A passing fork test is not a security audit.
2. Run `cd contracts && forge test -vvv`. Confirm the `GuardedDeploymentSafetyTest` and existing real-Aave fork tests **pass rather than skip**. Never publish a private HTTPS RPC endpoint.
3. Independently compile with Solidity **0.8.24**, pin compiler optimization settings and the **compiled creation bytecode + runtime bytecode hash**; do not accept an arbitrary compiled file or a changed contract.
4. Obtain the specific mainnet **deployment gasLimit, EIP-1559 maxFeePerGas, estimated USD cost** and separately approve this expense. **The $5 first-trade gas cap does not cover contract deployment.** No guessing or automatic broadcasting.
5. Verify MetaMask is on Ethereum Mainnet and the selected account exactly matches the designated owner/recipient above. Only **direct deployment from that wallet** makes the immutable `owner = msg.sender` match; deploying through a factory or another account creates a different owner.
6. Never paste a private key or recovery phrase into Remix, Render, GitHub, chat or a terminal. Use browser-wallet confirmation only. Use the official Remix IDE `https://remix.ethereum.org` if you choose manual wallet-signing; do not enter arbitrary proxy websites.

## Manual browser-wallet signing — only after explicit approval
- Open the reviewed `FlashLoanArbGuarded.sol` source in Remix, choose Solidity compiler 0.8.24, verify exact pinned compilation settings, and compile. Select **FlashLoanArbGuarded**, not the old `FlashLoanArb` contract.
- In Deploy & Run choose **Browser Extension** / connected MetaMask, chain ID 1, owner account `0x49826Aa476d6388fcdccAD7b3661597986a53132`.
- Enter constructor arguments exactly as listed above. Review the contract creation address preimage and wallet gas estimate. **Do not click Confirm until independently reviewed, within the owner-approved separate deployment budget.**
- If deployed, record tx hash, actual receipt, new **executor contract address** (different from owner wallet), block number, code hash and final EIP-1559 gas paid. An owner address is NOT an executor address.
- Keep `paused = true`, `autoPayout = false`, `maxLoanRaw(asset)=0` and `approvedPair(pair)=false`. Do NOT call setPaused(false), setAutoPayout(true), setLoanCap, setApprovedPair, or executeArbitrage during this stage.
- Verify source on Etherscan/Sourcify, query `owner()`, `pool()`, `profitRecipient()`, `paused()`, `autoPayout()` and independently match deployed runtime bytecode to locally reviewed compiled runtime. On-chain owner/recipient checks alone do NOT prove exact source.

## Separate connection to the *existing* Render worker
Only once deployment is independently confirmed, supply the PUBLIC values to the existing worker's environment:
```
GUARDED_EXECUTOR_ADDRESS=<verified deployed contract address>
EXPECTED_EXECUTOR_OWNER=0x49826Aa476d6388fcdccAD7b3661597986a53132
EXPECTED_PROFIT_RECIPIENT=0x49826Aa476d6388fcdccAD7b3661597986a53132
GUARDED_RUNTIME_KECCAK256=<independently pinned runtime bytecode hash>
```
The existing Rust `guarded_wiring` check verifies these read-only, on startup. **Never put a private key in the worker just because the identity check passes.** No signer currently exists and no trading is enabled.

## Hard no-trade blockers
- No actual verified profitable candidate after Aave premium, swap costs, price impact, gas and risk buffer (last observed `v2_gross_cycles=0`).
- One-shot ledger requires an approved durable database with a restricted INSERT/SELECT role and fixed singleton trial ID; existing available non-Nimbus Render free PostgreSQL expires 2026-12-05 and is owned by Trading Lab. Do not apply this migration there without verified isolation + continuity.
- Actual sender must be reviewed, single-writer nonce fenced, exact calldata simulation from the owner, enforce the $5 worst-case gas cap on the **exact signed transaction** with a fresh ETH/USD price, consume the durable trial atomically BEFORE broadcast and never retry an ambiguous broadcast.
- No direct wallet signing automation is configured. The user's MetaMask holds their seed/key; that key must never be exported to a cloud worker by default.
- No gas spending or transactions were authorized by creation of this document.

## GitHub Actions billing lock — local verification instead

The repository's GitHub Actions jobs currently **cannot start** because the GitHub
account is billing-locked. This is an external GitHub account issue, **not** a
Foundry test result, Solidity failure, or missing Cursor subscription. No
additional CI workflows or paid CI runners are required to verify locally.

From Windows PowerShell inside a complete local checkout:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-guarded-local.ps1
```

The script checks the constructor's owner/paused invariants and runs the
existing guarded tests with a locally installed Forge. If Forge is missing, it
exits with an explicit message and the official release link. If `HTTPS_URL`
is configured privately on the local machine, it also runs the mainnet-fork
suite; otherwise that fork suite is **not certified**. Never publish the URL.
The script cannot deploy or sign, and passing unit tests is not a security
audit. Keep this PR DRAFT until a genuine test record is available.

## Local, no-cost regression tests
```
cd contracts
forge test --match-contract GuardedDeploymentSafetyTest -vvv
```
These tests check immutable owner and recipient, initially paused and disabled asset/pair caps, and only-owner access. They neither deploy to mainnet nor prove current profitability.
