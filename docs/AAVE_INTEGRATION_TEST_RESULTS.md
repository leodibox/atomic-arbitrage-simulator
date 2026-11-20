# Aave executor integration validation — 2026-11-17

This is the earlier offline validation. See [the follow-up mainnet-fork report](AAVE_MAINNET_FORK_CERTIFICATION.md) for four executed and passing Aave/real-V2 fork tests at block 26436156.

Branch: `feature/guarded-aave-v2-executor-20261117`. Starting commit: `cf25a6ff872ffef65ab44b1d393bcff24c5b616b`. Changes in this validation do not modify `src/`, Rust manifests or the production scanner.

## Results

| Command | Result |
| --- | --- |
| `node --test scripts/check_aave_wiring.test.mjs` | PASSED: 6 |
| `cd contracts && forge build` | PASSED |
| `cd contracts && forge test -vvv` | PASSED: 29; FAILED: 0; SKIPPED: 12 |
| `forge test --match-contract AaveMainnetForkTest -vvv` | SKIPPED: 2; HTTPS_URL unavailable |
| `cargo test --locked --all-targets` | PASSED: 15 |

Foundry 1.7.1 (`4072e48705af9d93e3c0f6e29e93b5e9a40caed8`), Solc 0.8.24, forge-std `15b2df92a05d4d2f76a06653872fefb287e0e01c`. Solc SHA256 checked against the official ethereum/solc-bin manifest: `fb03a29a517452b9f12bcf459ef37d0a543765bb3bbc911e70a87d6a37c30d5f`. Node 24.22.1; Rust 1.100.0. Native forge was used so its exit status is preserved. In this workspace activate with `export PATH=/workspace/.tools/bin:$PATH FOUNDRY_SOLC=/workspace/.tools/bin/solc-0.8.24`. A normal Foundry installation can use its own verified Solc.

## Verified changes and behavior

- Fixed the reproduced default-build `Stack too deep` error by limiting swap-loop local variable lifetimes; no compiler-profile or business-logic change.
- Older fork suites now mark each test SKIPPED when HTTPS_URL is absent. A configured but unusable endpoint still fails; a non-mainnet endpoint fails the chain check.
- Mock tests now use the confirmed immutable receiving wallet `0x49826Aa476d6388fcdccAD7b3661597986a53132`.
- Verified exact principal plus 5-bps mock premium, exact post-premium minimum profit, fully consumed repayment allowance, preservation of pre-existing balances, and payout only after the pool completes repayment.
- Failed input transfer, approval, repayment and wallet payout restore wallet, executor, pool, pair balances and reserves.
- Checked owner restrictions, callback caller/activity checks, approved pairs, default pause/payout state and loan limits. Payout fuzz test passed 256 runs over loan amount and pre-existing float.

## Limitations and remaining risks

- No configured RPC variable or secure binding was available. Canonical Ethereum Aave V3 Pool `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2`, live liquidity, actual premium, USDC behavior, deployed executor bytecode/owner and actual wallet wiring remain unverified. The Node verifier used fake RPC responses. No claim of successful mainnet integration.
- Mock AMMs have synthetic reserves and do not enforce a canonical constant-product invariant. Tests prove receiver accounting for those fixtures, not real executable arbitrage. The existing mainnet fork also uses synthetic AMMs.
- Only audited/approved 30-bps V2 pairs and supported ERC20 assets are appropriate. Fee-on-transfer, rebasing, malicious tokens/pairs and broader callback/reentrancy behavior require further review.
- Pool address is constructor-supplied; nonzero code is checked, but deployment must independently enforce the canonical chain/pool identity. Owner and pair approvals remain trust boundaries.
- Minimum profit is borrowed-asset profit after premium; it excludes ETH gas, slippage after simulation, MEV and nonce/signing risks. A failed mainnet transaction would still cost gas, despite reverted token transfers.
- Automatic payout is opt-in and disabled on construction; pause is enabled and loan caps/pair approvals disabled by default. Tests only enable these on disposable local fixtures.
- Legacy FlashLoanArb.sol still emits unchecked ERC20 return-value warnings; production legacy code was not changed. These findings are not a clean security audit of the legacy executor.
- GitHub API access returned Forbidden. PR #10 draft state and CI status could not be independently verified; no ready/merge/deploy action was taken. No real transactions, deployments, approvals or private keys were used.

## Individual Solidity test results

All SKIPPED entries below require an Ethereum mainnet RPC and were not passed.

### test/AaveMainnetFork.t.sol:AaveMainnetForkTest

| Test | Status |
| --- | --- |
| `testFork_readsRealAaveFlashPremium()` | SKIPPED |
| `testFork_realAaveLoanRepaymentAndWalletPayout()` | SKIPPED |

### test/FlashLoanArb.t.sol:FlashLoanArbTest

| Test | Status |
| --- | --- |
| `test_badPathReverts()` | SKIPPED |
| `test_ownerIsDeployer()` | SKIPPED |
| `test_poolAddress()` | SKIPPED |
| `test_profitRecipient()` | SKIPPED |
| `test_sweepUnauthorizedReverts()` | SKIPPED |
| `test_unauthorizedCallerReverts()` | SKIPPED |
| `test_unauthorizedExecuteOperationReverts()` | SKIPPED |
| `test_unprofitableReverts()` | SKIPPED |
| `test_zeroProfitRecipientReverts()` | SKIPPED |

### test/FlashLoanArbGuarded.t.sol:FlashLoanArbGuardedTest

| Test | Status |
| --- | --- |
| `testFuzz_payoutPreservesFloat(uint64,uint64)` | PASSED |
| `test_autoPayoutDisabledUntilOwnerExplicitlyEnables()` | PASSED |
| `test_autoPayoutDoesNotTransferIfWholeFlashloanReverts()` | PASSED |
| `test_autoPayoutOnlyAfterAaveRepayment()` | PASSED |
| `test_constructorPausedAndDisabledByDefault()` | PASSED |
| `test_directCallbackRejected()` | PASSED |
| `test_emptyReturnApprovalSupported()` | PASSED |
| `test_exactPrincipalPremiumAndConfirmedWalletPayout()` | PASSED |
| `test_failedAaveRepaymentNeverPaysWallet()` | PASSED |
| `test_failedApprovalRollsBack()` | PASSED |
| `test_failedInputTransferRollsBack()` | PASSED |
| `test_failedWalletTransferRollsBackLoanAndSwaps()` | PASSED |
| `test_minProfitAboveNetAfterPremiumRevertsAtomically()` | PASSED |
| `test_nonOwnerCannotExecuteSweepOrSetLoanCap()` | PASSED |
| `test_nonOwnerCannotUnpauseOrAuthorizePair()` | PASSED |
| `test_onlyOwnerCanSetAutoPayout()` | PASSED |
| `test_pausedByDefaultAndEmergencyPause()` | PASSED |
| `test_poolCannotCallOutsideActiveLoan()` | PASSED |
| `test_profitableApprovedV2PathAndSweep()` | PASSED |
| `test_rejectDuplicatePair()` | PASSED |
| `test_rejectInsufficientProfit()` | PASSED |
| `test_rejectInvalidDirectionByte()` | PASSED |
| `test_rejectMissingMinProfit()` | PASSED |
| `test_rejectOverLoanCap()` | PASSED |
| `test_rejectUnapprovedPair()` | PASSED |
| `test_rejectWrongDirection()` | PASSED |
| `test_walletRemainsUnpaidUntilPoolFinishesRepayment()` | PASSED |
| `test_zeroAmountAndDisabledAssetRejected()` | PASSED |

### test/FlashLoanArbProfit.t.sol:FlashLoanArbProfitTest

| Test | Status |
| --- | --- |
| `test_profitablePathRepaysAndKeepsSurplus()` | PASSED |

### test/Sandooo.t.sol:SandoooTest

| Test | Status |
| --- | --- |
| `test_staleBlockReverts()` | SKIPPED |

## Individual Node test results

| Test | Status |
| --- | --- |
| read-only verifier confirms pinned mainnet wiring but cannot enable trades | PASSED |
| recipient mismatch is a hard verification failure | PASSED |
| deployment missing bytecode fails closed | PASSED |
| wrong chain fails before inspecting contracts | PASSED |
| unverified owner or bytecode cannot be treated as connected | PASSED |
| legacy contract without paused() fails closed | PASSED |

## Individual Rust regression results

| Test | Status |
| --- | --- |
| `arbitrage::cycles::tests::finds_profitable_two_pool_cycle` | PASSED |
| `arbitrage::preflight::tests::refuses_noncanonical_v2_fee_and_modified_quote` | PASSED |
| `arbitrage::preflight::tests::refuses_negative_profit_or_excessive_gas_budget` | PASSED |
| `arbitrage::preflight::tests::refuses_stale_state_and_oversized_loan` | PASSED |
| `arbitrage::preflight::tests::builds_v2_only_contract_calldata` | PASSED |
| `arbitrage::preflight::tests::refuses_unpriced_gas` | PASSED |
| `arbitrage::preflight::tests::refuses_v3_or_missing_pair_in_executable_path` | PASSED |
| `arbitrage::state::tests::quotes_with_fee_and_updates_reserves` | PASSED |
| `arbitrage::state::tests::rejects_stale_sync_updates` | PASSED |
| `arbitrage::state::tests::v3_one_for_zero_converts_not_passes_through` | PASSED |
| `arbitrage::state::tests::v3_rejects_tokens_not_in_pair` | PASSED |
| `arbitrage::topology::tests::can_form_two_hop_v2_v3_closed_route` | PASSED |
| `arbitrage::topology::tests::disjoint_pools_cannot_form_cycles` | PASSED |
| `scanner_tests::eighteen_decimal_amount_units` | PASSED |
| `scanner_tests::six_decimal_amount_units` | PASSED |
