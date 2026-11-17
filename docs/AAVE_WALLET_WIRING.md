# Aave V3 + receiving wallet — verification and payout integration

This draft PR prepares the **Ethereum mainnet Aave V3** flash-loan contract and a **read-only connection verifier**. It cannot transact until the separate execution/signer gates pass. Do not paste RPC secrets or private keys into chats, GitHub files, or logs.

## What is and is not a wallet connection?

The address in `.env.example` is only an example of the intended **ERC-20 recipient**, not evidence that Render is configured, that its owner controls the destination, or that the user owns the MetaMask address. A public Ethereum address needs no login or private key to **receive** tokens. A separate owner/signer key is needed to deploy the contract, configure caps and submit transactions.

- Confirm in MetaMask **Account details** that the receiving address equals the intended `PROFIT_RECIPIENT_ADDRESS` (currently `0x49826Aa476d6388fcdccAD7b3661597986a53132` in the example).
- Confirm chain ID `1` (Ethereum Mainnet). Ethereum mainnet Aave V3 Pool: `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2` (verify against https://aave.com/docs/aave-v3/markets/data).
- Do **not** assume `BOT_ADDRESS` is a deployed receiver. A wallet `PRIVATE_KEY` in Render would **not** magically enable transaction submission; the Rust scanner remains read-only.
- `profitRecipient` is set in the contract constructor and immutable. Wrong address means deploy a new, independently verified contract before trading.

## Automatic profits to MetaMask (opt-in)

`FlashLoanArbGuarded.sol` has `setAutoPayout(bool)` (owner only; **disabled by default**). When the owner enables it, `executeArbitrage` reads the starting borrowed-token balance, performs a guarded Aave `flashLoanSimple`, waits for Aave to **pull principal + premium**, checks the increase in contract balance, and **only then** transfers the realized positive surplus to the immutable `profitRecipient`. If the swap or repayment reverts, the transaction is atomic and no payout occurs. Prior contract token balance is not part of the automatic payout. `sweep()` remains an owner-only fallback.

A profitable trade borrowing USDC pays the wallet **USDC**, not automatically ETH. MetaMask may need the token imported to display it. The owner's ETH gas cost is paid from the signing wallet and must be independently subtracted to calculate **real net profit**. The on-chain `minProfit` is denominated in the loaned token, not ETH.

## Read-only on-chain wiring verification

After a separately approved, independently reviewed mainnet deployment and with Node.js 18+ locally or on a trusted development machine:

```sh
# Set these privately on your machine; do not paste the values into chat.
export HTTPS_URL="https://...your-alchemy-mainnet-https-endpoint..."
export EXECUTOR_ADDRESS="0x...deployed-reviewed-executor..."
export EXPECTED_PROFIT_RECIPIENT="0x...YOUR_CONFIRMED_METAMASK_ADDRESS..."
export EXPECTED_OWNER="0x...YOUR_AUTHORIZED_DEPLOYER_OR_SIGNER_ADDRESS..."
export EXPECTED_RUNTIME_CODE_SHA256="...verified-reviewed-compiled-runtime-hash..."

node --test scripts/check_aave_wiring.test.mjs
node scripts/check_aave_wiring.mjs
```

The verifier uses only `eth_chainId`, `eth_getCode`, and `eth_call`. It checks:
- chain ID 1 and official Aave Pool code;
- deployed executor bytecode present **and SHA256 pinned against an independently reviewed compiled runtime**;
- `pool()` equals Aave Ethereum Pool;
- `owner()` matches the expected signer;
- `profitRecipient()` matches the wallet address;
- `paused()`, `autoPayout()`, and the Aave `FLASHLOAN_PREMIUM_TOTAL()` getter.

Failure exits with status 2. Passing means **read-only contract wiring is consistent**, NOT that real-money trading is safe or enabled. Do not unpause or enable payout based on this report alone.

## Mainnet-fork integration test (no real transaction)

`contracts/test/AaveMainnetFork.t.sol` provides two tests against a local
**fork** of the real Ethereum state: it reads Aave's actual flash-loan premium,
and calls `flashLoanSimple` for real USDC in the fork while swapping through
**synthetic local V2 pairs**, repaying Aave and checking that the immutable
recipient receives only surplus. The dummy pair price gap is deliberately
artificial: passing this test would establish Aave callback/repayment compatibility,
**not** a profitable trade in the real market.

```sh
cd contracts
# Optional without RPC: fork tests are explicitly SKIPPED, not verified.
forge test -vvv
# With private HTTPS_URL set in your local environment, run the actual fork:
forge test --match-contract AaveMainnetForkTest -vvv
```

Check test results for **passed vs skipped**. A skipped fork test is never
evidence of a working Aave integration. Do not put HTTPS_URL credentials in
GitHub, a public PR, or a screenshot.

`scripts/check_aave_wiring.mjs` selector identifiers were independently
recomputed using Ethereum Keccak-256 and matched all six getters, but neither
the live executor's chain-code hash nor wallet ownership is verified until the
read-only check runs against the actual deployed address.

## GitHub Actions runner limitation

GitHub Actions for this repository has failed before starting a usable Foundry
job, and there is currently no trusted `forge`/Solidity compiler available in
this chat runtime. The workflow also includes the dependency-free Node verifier
tests. The PR therefore remains in **draft** with no production deployment.
The repository owner should check **GitHub Settings → Billing and licensing**
and **Settings → Actions → General**, or use an already configured trusted
Foundry development computer to produce the real test/compile results. Do not
merge based only on source inspection.

## Solidity and Aave execution gates

- Install a working Foundry toolchain and forge-std (the current GitHub Actions Foundry job failed without usable logs); run `cd contracts && forge build && forge test -vvv`, then Ethereum-mainnet **fork** and fuzz/invariant tests.
- Independently review the guarded receiver, reentrancy assumptions, ERC-20 behavior, and pairs approved through `setApprovedPair`. No Uniswap V3/V4 paths are executable by this V2-only contract.
- Verify deployed bytecode, pool, owner, recipient, flash liquidity and actual premium, plus that candidate routes are 30 bps V2-compatible at the latest block.
- Implement a **single-writer nonce-fenced signer**, executable-route verification, `eth_call`, `eth_estimateGas`, independently priced ETH→asset gas, transaction deadline, max transaction fee, hard daily gas/loss budget, emergency stop, and receipt-based realized P&L.
- Owner must authorize spending caps and use a dedicated, funded wallet privately. Never ask for the private key in chat.
- Run a separately approved low-loss capped live canary **only after** all gates above pass and a genuinely positive post-gas candidate exists.

This PR is **draft only**: it does not change production Render environment, deploy a smart contract or submit any mainnet transaction.
