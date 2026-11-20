# Automatic one-shot Aave execution — release gates

Status: **NOT LIVE**. Owner requires automatic execution without per-trade approvals, with a **maximum total of US$5 in gas for the first trial**, exactly **one attempted submission**, and **no retries**. This document does not approve funding, signing or contract deployment.

## Existing pieces

- `src/arbitrage/net.rs` scans canonical WETH V2 paths, computes preliminary net estimates and invokes `prepare_v2_flash_loan` for compatibility.
- `src/arbitrage/preflight.rs` produces `executeArbitrage(address,uint256,bytes,uint256)` calldata; it does not sign or broadcast.
- `contracts/src/FlashLoanArbGuarded.sol` is the integrated, **paused-by-default** candidate. No contract address, approved pairs, signer or funded execution was verified.
- `src/arbitrage/one_shot.rs` is a **pure guard** to be called only by a verified execution coordinator. It does not provide a sender, persistence, price oracle, RPC simulation or trade authorization. This branch must remain separate from the deployed read-only worker until tested.

## Exact automatic execution prerequisites — fail closed

1. Before candidate assessment, obtain a verified, fresh ETH/USD oracle quote (8 decimals), with source, round and timestamp; do not accept an environment-supplied static price as live proof. Reject stale, invalid or mismatched-chain quotes.
2. Find a **net-positive, executable** V2 route. Reprice using latest/resolved block; account for swap fees, Aave premium, slippage, gas, loan limits and MEV/inclusion effects. Gross profit is not a signal.
3. Identify the deployed **Guarded executor** by Ethereum mainnet chain ID, actual bytecode, owner, canonical Aave pool, immutable profit recipient, current pause flag, pair approvals, asset/loan cap and exact sender. Do not use the legacy executor as an implicit fallback.
4. Prepare calldata with the confirmed ABI and exact sender; run `eth_call` and gas estimation against the candidate transaction. Do not treat a simulation on synthetic reserves as a real route.
5. Require `gasLimit * maxFeePerGas * verifiedEthUsd`, conservatively rounded up plus 10% margin, <= US$5 **for the entire authorized trial**, not merely per retry. The signed transaction must use the exact validated caps. Reject missing/overflowing/old inputs and gas funding shortfalls. An ETH/USD price move after authorization can still change realized USD cost; the margin reduces but cannot eliminate this risk.
6. **Atomically reserve and consume** the trial's only attempt in durable storage BEFORE any broadcast. Store an immutable attempt ID, chain, signer, nonce, calldata hash, gas limit, fee caps, and transaction hash when available. Restarting the worker or running another instance MUST NOT reset the quota; if durable state or exclusive reservation is unavailable, reject. Do not automatically replace-by-fee, retry failed broadcasts, or resubmit ambiguous receipts.
7. Verify nonce ownership, signer custody, nonce fencing, wallet ETH for maximum gas, and explicit owner-level enablement; contract remains paused until a separate reviewed operational activation. No secrets in logs or commits.
8. Only then may an approved sender sign and broadcast **once**, without an interactive approval for each trade. Poll the receipt read-only; calculate actual ETH gas spent and realized token surplus. After attempt or uncertain broadcast, lock the execution mode until an explicit new budget authorization.
9. Have a tested emergency stop, signer rotation procedure and an approved maximum exposure. **A US$5 gas cap does not bound losses from a compromised signer, a malicious token, or an incorrect contract.**

## Current hard blockers

- No live Guarded contract deployment verified; on-chain owner/permissions cannot be certified.
- No validated signer, funded gas wallet, current exact-calldata successful simulation or profitable candidate.
- No durable one-attempt ledger, verified live ETH/USD quote feed or transaction submission code attached to the existing read-only scanner.
- Local build/tests of this branch have not yet been executed by the GitHub connector. GitHub Actions is blocked by billing; use local Codex/Foundry or the existing Render build gate WITHOUT enabling execution.

Until those checks pass, the **only safe deployed state** is read-only scanning. Do not silently change `POOL_SCAN_ENABLED` to `true`: that switch selects a different legacy pending-transaction strategy, not guarded Aave auto-execution.
