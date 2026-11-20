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
- The PostgreSQL one-attempt ledger **schema is drafted** at `sql/first_live_trade_attempt.sql`, but it is **not deployed**, connected or proven transactional end-to-end. No dedicated Sandwich Bot PostgreSQL instance was found in the connected Render workspace. Existing Postgres/Redis instances belong to Nimbus or Trading Lab and must not be reused without explicit isolated ownership and approval. No verified live ETH/USD quote feed or transaction submission code is attached to the existing read-only scanner.
- Local build/tests of this branch have not yet been executed by the GitHub connector. GitHub Actions is blocked by billing; use local Codex/Foundry or the existing Render build gate WITHOUT enabling execution.

Until those checks pass, the **only safe deployed state** is read-only scanning. Do not silently change `POOL_SCAN_ENABLED` to `true`: that switch selects a different legacy pending-transaction strategy, not guarded Aave auto-execution.


## Dedicated persistent attempt ledger — implementation contract

The SQL migration file `sql/first_live_trade_attempt.sql` also uses a database trigger to reject stale/future ETH/USD quotes and independently recompute maximum gas cost in USD with a 10% safety margin (it is not an oracle). It uses a single immutable primary key (`first_live_trial`) and `INSERT ... ON CONFLICT DO NOTHING RETURNING attempt_id`. It protects at-most-one **only** if:

- the exact signed transaction has already passed all simulation and price checks;
- a dedicated PostgreSQL database stores the row durably;
- the writer commits the reservation *before* broadcasting and verifies commit success;
- the worker's DB role has only SELECT/INSERT privileges, no DELETE/UPDATE/TRUNCATE and no table ownership;
- execution never invents another trial ID, resets the row, or broadcasts when the DB is unavailable;
- no second signer/coordinator can bypass the database or reuse a nonce;
- the contract's paused/owner/cap/pair state is independently checked immediately before authorization.

No database or secret has been provisioned or modified by this branch. **Do not treat a SQL schema as an active safety limit.**

## Exact transaction simulation — evidence protocol

Before any live activation, require a *real* positive-net candidate with current block, pair direction, size and same-block reserves. Independently confirm:

1. Real deployed `FlashLoanArbGuarded` address, reviewed runtime bytecode hash, on-chain `owner()`, `pool()`, `profitRecipient()`, `paused()`, approved pairs, `maxLoanRaw(asset)`, and dedicated signer.
2. Exact `executeArbitrage(address,uint256,bytes,uint256)` calldata from `preflight.rs`, with the correct guarded executor ABI and signed gas caps.
3. `eth_call` and `eth_estimateGas` at a current state using the **actual owner sender and executor**. A paused contract must reject; an off-chain fork may be used for pre-activation testing but is not a live canary.
4. USD gas conversion from independently verified ETH/USD market data and EIP-1559 `maxFeePerGas`, with the ten-percent safety margin.
5. A test that a profitable positive in mocks cannot bypass the above, and a real unprofitable route is rejected; no relaxed conditions in production.

No profitable candidate or deployed Guarded executor has been verified as of this document. Do not fabricate an address or interpret fork-only synthetic profits as revenue.


## Isolated database test (NOT run yet)

Run locally only with a **disposable database** whose name starts with `sandwich_bot_test_`, a separate test-admin PostgreSQL role, and a privately supplied `PGHOST`/`PGPORT`/`PGUSER`/`PGPASSWORD`:

```sh
export SANDWICH_TEST_DB_NAME=sandwich_bot_test_local
node scripts/test_one_shot_ledger.mjs
```

The Node test requires the `psql` command; it will refuse other database names. It applies the migration and **TRUNCATEs** the attempt/receipt tables in that disposable database only. Its checks cover two concurrent reservations, persistence across separate sessions, rejection of a second attempt, mismatched/over-$5 quoted fee, stale/future quote, and one accepted reservation. This test has not been executed in the connected environment because there is no approved dedicated disposable Sandwich Bot PostgreSQL instance or local `psql` binary.

The Rust risk policy was corrected to perform `assess_before_reservation` prior to consuming the quota, and `assess_after_reservation` only after an independently verified committed singleton reservation. The coordinator must match the persisted row to the exact calldata/nonce/sender and MUST NOT trust a user-provided boolean as proof of DB durability.
