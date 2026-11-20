# Restricted trade operator — proposal; not activated

**Status:** draft implementation for local Foundry review. This changes
`FlashLoanArbGuarded.sol` runtime bytecode and therefore INVALIDATES earlier
deployment estimates, runtime hashes and any notion of prior completed audit.

## Intended roles

- **Owner and profit recipient:** `0x49826Aa476d6388fcdccAD7b3661597986a53132`.
  Deploy directly from this EOA; immutable owner is `msg.sender`.
- **Operator:** a future SEPARATE, low-value, narrowly funded execution
  account. **No address has been selected or authorized.**
- Operator authorization defaults to **false**. Only the owner can call
  `setApprovedOperator(operator, true)` and revoke it.
- Approved operator can call ONLY `executeArbitrage()` in addition to
  unprivileged getters. The owner exclusively retains `sweep`, pausing,
  pair approval, loan caps, auto-payout changes and operator authorization.
- Profits in the token stay inside the contract until the owner sweeps,
  unless the owner independently enables `autoPayout` (off by default).
  ETH gas is still paid by the account that actually submits the transaction.

## Critical distinction: not yet a safe automatic bot

1. An approved operator can **spend its own ETH gas on attempts repeatedly**.
   Contract profit checks revert unprofitable flash loans, but Ethereum gas is
   burned even if they revert. The contract alone does NOT enforce a $5 gas cap
   or a single unsuccessful attempt.
2. An off-chain sender must enforce an exact-calldata fresh-chain `eth_call`,
   nonce fencing, fees and positive net profit after gas, with durable one-shot
   transaction reservation committed BEFORE any potential broadcast. A public
   read-only scanner does NOT fulfill these requirements.
3. The operator's separate signing key must never be pasted into GitHub, chat,
   the repo, or generic Render environment variables. A secure signer/permission
   arrangement and limited ETH funding need independent owner approval.
4. Owner's current MetaMask key must never be exported to an automated worker.
   The owner wallet remains both controller and immutable profit recipient.
5. An approved operator must be revoked by the owner on compromise. Emergency
   pause blocks every executor. Owner must separately approve pair/loan caps.
6. Modifying the actual contract requires REPEAT local/fork regression tests,
   external review and fresh bytecode/runtime hash + `eth_estimateGas`.
   The prior **2,617,633 deployment-gas estimate applied to older bytecode**.
7. Contract has NOT been deployed; this PR does NOT authorize deployment,
   approvals, unpause, transfers or production enablement.

## Commands (PowerShell 5.1, Foundry already installed)

From a checked-out copy of this branch:

```powershell
cd "$HOME\Downloads\atomic-arbitrage-simulator"
git fetch origin
git switch feature/guarded-restricted-operator-20261120
git pull --ff-only origin feature/guarded-restricted-operator-20261120
cd contracts
$env:PATH = "$HOME\Downloads\atomic-arbitrage-simulator\tools\foundry;$env:PATH"
forge test --match-contract GuardedRestrictedOperatorTest -vvv
forge test --no-match-contract AaveMainnetForkTest -vv
$env:HTTPS_URL = "https://ethereum-rpc.publicnode.com"
Remove-Item Env:MAINNET_FORK_BLOCK -ErrorAction SilentlyContinue
forge test --match-contract AaveMainnetForkTest -vvv
```

Pass with **no failures and no skipped fork tests** before marking this
change ready. If RPC history is pruned, use a supported recent fork block
rather than pretending fork tests passed. Do not automatically rerun tests
that themselves failed for contract reasons.

## Work not included in this PR
- No generated or stored credentials; no separate operator account.
- No database or persistent one-shot ledger activated.
- No real order sender, no gas spending, no trade execution.
- No change to the Rust Render worker, Nimbus or Trading Lab.
