# Zero-new-Render-service one-shot budget — GitHub ref research

**Status: experimental, not approved for mainnet spending.** This is a zero-new-service fallback for the first $5-gas-budget attempt, not a general-purpose durable trading queue or a replacement for a dedicated restricted DB when automating many trades.

## Why not the existing PostgreSQL/Redis?
Render currently has **no dedicated Sandwich Bot database**. The available non-Nimbus database, `trading-lab-history`, is free, belongs to Trading Lab, and reports expiry on 2026-12-05. It also denies remote SQL from the hosted Render MCP. Using it without isolated privileges and rollover would risk another project. The Trading Lab Redis is configured with persistence **off** and `allkeys_lru`, unsuitable for permanent one-shot state. A new free Render PostgreSQL was attempted and rejected: there is already an active free database. **No new database was provisioned.**

## Research prototype
`scripts/github_one_shot_claim.mjs` creates the ONE global Git ref:

```
refs/heads/first-live-gas-budget-consumed
```

It uses a single `POST /repos/leodibox/atomic-arbitrage-simulator/git/refs` with a pinned reviewed Git commit SHA, and a `GET` read-back. A duplicate fixed-ref creation should receive HTTP 422. Any network error, unexpectedly successful overwrite, missing read-back, wrong SHA, ambiguous status, or rate limit means **no signed transaction**. No retry, force-update or delete path exists in the module.

**Important limitations**:
- Requires a secure dedicated GitHub token with narrowly scoped repository permissions in existing Render environment; no secrets in GitHub, chat, or logs. Creating the token is a user-controlled step.
- The fixed ref is **deletable by GitHub admins unless protected by an effective branch ruleset**. Protecting against deletion and bypasses must be independently verified; this code currently receives only a boolean assertion, which is NOT evidence. Do not use live until that is replaced by verified ruleset proof and admin bypass risk acceptance.
- The ref is a permanent *trial-spent* marker; once created, it cannot be reused even if no opportunity or no transaction happened. For low latency, do pre-arming **after** candidate and exact simulation pass; network latency can make the opportunity stale. Never send stale transactions just because claim succeeded.
- A successful claim does NOT ensure a legitimate sender, profitable exact transaction, verified deployed Guarded contract or enforced $5 EIP-1559 fee cap. All these are separate prerequisites.
- GitHub audit/retention and API availability are independent of Render, but do not guarantee cryptographic immutability against repository administrators. If it cannot be protected, use a restricted dedicated persistent database instead.
- A single successful broadcast still needs receipt auditing, emergency stop, and no signer fallback. Never enable `POOL_SCAN_ENABLED=true`; this starts an unrelated legacy strategy.

## Verification
A minimal mocked transport test is in `scripts/github_one_shot_claim.test.mjs`. It never contacts real GitHub or submits blockchain transactions. Run in Node 18+ with:

```sh
node --test scripts/github_one_shot_claim.test.mjs
```

The tests check: first ref claim and matching read-back, duplicate 422 with no retry, timeout with no retry, mismatched SHA, and refusal on missing preconditions. These tests are **not yet a mainnet safety certification**. A separate review of concurrency against real GitHub test refs, without consuming the production fixed reference, is required before use. Do NOT create the permanent production ref during tests.

## Mainnet execution still blocked
The scanner still has no verified positive-net compatible route. There is no independently verified deployment address and bytecode for `FlashLoanArbGuarded`, no dedicated signer, exact-sender successful simulation, live ETH/USD oracle, protected GitHub ruleset, or sender connected to the scanner. **Do not merge or deploy this experiment into a live sender until those checks and user activation are done.**
