# Aave V3 mainnet-fork and real V2 route validation

Validation date: 2026-11-17. Branch: `feature/guarded-aave-v2-executor-20261117`.

## Scope and RPC provenance

No HTTPS_URL was exported, no local .env was present, and the managed environment reported no secret/variable bindings. A temporary **public PublicNode HTTPS** RPC was therefore used without credentials. `eth_chainId` returned `0x1`. This was not a newly discovered private endpoint. No URL with credentials, private key or secret was stored or printed.

Finalized fork block: **26436156**, hash `0xd4a913774c2aeb75968d823dc020734258e65ae8c488df382fa0669646dd1fd8`. An optional `MAINNET_FORK_BLOCK` now pins createSelectFork to a reproducible block. It does not modify reserves/state. With no override, the prior latest-block behavior is retained.

Foundry 1.7.1, verified Solc 0.8.24, forge-std `15b2df92a05d4d2f76a06653872fefb287e0e01c`. No production Solidity or Rust scanner files were changed in this follow-up; changes are fork tests and documentation.

## Exact fork results

Executed `forge test --match-contract AaveMainnetForkTest -vvv` with HTTPS_URL set only in the test process and MAINNET_FORK_BLOCK=26436156.

| Test | Final status | Evidence |
| --- | --- | --- |
| `testFork_readsRealAaveFlashPremium()` | PASSED | chain ID 1; canonical Pool bytecode and real USDC bytecode; actual premium 5 bps |
| `testFork_realAaveLoanRepaymentAndWalletPayout()` | PASSED | canonical Pool callback and real USDC; borrowed 10,000 USDC, pulled 10,005 USDC into the reserve aToken; liquidity increased by 5 USDC; allowance consumed |
| `testFork_realV2SushiThenUniswap()` | PASSED | both real pair swap calls executed; quote below principal + premium; expected Unprofitable revert and state rollback |
| `testFork_realV2UniswapThenSushi()` | PASSED | both real pair swap calls executed; quote below principal + premium; expected Unprofitable revert and state rollback |

**Final fork total: 4 PASSED, 0 FAILED, 0 SKIPPED.** The same four passed in the gas-report run.

The initial latest-block run had **1 PASSED, 1 FAILED, 0 SKIPPED**. The failure was an RPC `block not found` backend error while loading state. It was not a demonstrated executor defect. Repeating against a fixed finalized block succeeded. The report does not count that initial failure as a passed run.

The official Aave V3 Ethereum Pool tested is `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2`. Its getReserveData(USDC) supplied the reserve aToken address. The successful test expected the precise USDC transferFrom(executor, aToken, principal + actual premium), verified the liquidity balance increase, and verified zero remaining repayment allowance.

The confirmed wallet `0x49826Aa476d6388fcdccAD7b3661597986a53132` received exactly **9,298.953512 USDC** and the executor retained zero new USDC in the successful fixture. **This gain is artificial:** that fixture funds synthetic AMM reserves with fork-only deal/mint. It proves Aave repayment and wallet integration, not a real profitable opportunity. Mock-only regression tests separately observe payout timing after repayment and full rollback when payout/repayment fails.

## Real pools and tokens, unchanged reserves

Two additional tests use real USDC/WETH contracts and actual deployed pair bytecode/reserves:

- Uniswap V2 USDC/WETH: `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc`.
- SushiSwap V2 USDC/WETH: `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0`.

The tests verify token0/token1, canonical factory addresses, factory getPair identity, nonzero reserves, both exact swap calldata calls, the Aave premium, and receiver/payout/allowance state. These route tests use **no deal, mint, storage overwrite, reserve manipulation or external funding**. The flash loan is provided solely by the canonical on-fork Aave Pool.

| Route | Input USDC | Real reserve quote USDC | Aave premium USDC | Net before gas USDC | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| Sushi → Uniswap | 100 | 99.357077 | 0.05 | -0.692923 | expected revert; wallet/liquidity/reserves unchanged |
| Uniswap → Sushi | 100 | 99.270077 | 0.05 | -0.779923 | expected revert; wallet/liquidity/reserves unchanged |

Passing these tests means the executor correctly rejects these unprofitable real routes. It does not mean a successful profitable trade was demonstrated. Other tokens, pair implementations, loan sizes, multi-hop paths and blocks remain untested by this campaign.

## Gas observations and estimate limits

Measured at the pinned block with the repository's unoptimized compiler profile:

| Measurement | Gas | Meaning |
| --- | ---: | --- |
| Real-pair reverting route, normal test gasleft delta | 281,795 | warm in-test execution, including test-call overhead; excludes transaction intrinsic gas |
| Synthetic-AMM successful Aave/payout, normal gasleft delta | 332,373 | artificial profitability, warm in-test execution |
| executeArbitrage gas-report instrumentation | 357,265–362,667 | three calls; instrumentation changes cold-access accounting |
| Receiver deployment, gas-report | 2,595,864 | local constructor measurement; no deployment performed on Ethereum |

These are diagnostic gas baselines, **not an exact production eth_estimateGas result or safe gas limit**. Calls made earlier in a test warm accounts/storage; fixture and gas-report instrumentation affect accounting. Reported per-test totals also include fixture deployment/setup and must not be mistaken for transaction gas. Both real routes revert, so there is no successful real profitable calldata for which to certify a sendable gas estimate.

The block base fee was 476,060,689 wei/gas (~0.4761 gwei). As an illustrative calculation only, adding a 1-gwei priority fee to the 281,795 warm execution observation gives ~0.000415947 ETH, excluding intrinsic/calldata gas, cold-access differences, future fee changes and any extra safety margin. No ETH→USDC gas conversion or post-gas profit claim is made. A fresh exact-calldata eth_call and eth_estimateGas with the intended sender, nonce/fee assumptions and latest state remain mandatory before any separately authorized deployment/execution.

## Remaining security and deployment requirements

- No independent audit, user wallet-control attestation, production deployment, reviewed runtime-code match or owner/signer setup has been certified. Constructor-supplied Pool must be independently checked for the intended chain; nonzero code alone is not sufficient deployment attestation.
- Only canonical 30-bps V2 pairs were exercised here. Approved pairs/assets remain an owner trust boundary. Nonstandard/rebasing/fee-on-transfer tokens, malicious callbacks, alternative AMMs and broader reentrancy/invariant campaigns need separate review.
- Actual Aave liquidity, premium, reserve status, USDC proxy/blacklist behavior and pair state can change after this block. Public RPC can be inconsistent; use an authenticated production-grade archive-capable endpoint privately, verify chain and block, and do not trust one historical run as a live guarantee.
- Minimum profit protects borrowed-asset surplus after premium; ETH gas, stale quotes, slippage, MEV and inclusion risk are additional costs. Reverted on-chain transactions still consume gas even when token transfers roll back.
- Preserve default pause, disabled automatic payout, zero asset caps and unapproved pairs until separately reviewed. Require dedicated signer custody, single-writer nonce fencing, exact fresh simulation, fee/deadline limits, independently priced gas, daily loss budgets, emergency stop and receipt-based P&L before live use.
- This run does not authorize deployment, unpausing, spending approvals or real-money trading. All deployments, approvals, transfers and swaps in the tests occurred in the local disposable EVM fork. No signed transaction, broadcast or real fund movement occurred.

## Regression checks

Offline `forge test -vvv`: **29 PASSED, 0 FAILED, 14 SKIPPED**. The 14 skips are expected without HTTPS_URL in that separate process (four Aave fork tests plus ten legacy fork tests). They are not counted as passes. The four Aave fork tests were then independently verified above with RPC configured. Node wiring unit tests: **6 PASSED, 0 FAILED, 0 SKIPPED**. Rust source/manifests remain byte-for-byte unchanged in this follow-up.

## Offline individual test statuses

### test/AaveMainnetFork.t.sol:AaveMainnetForkTest

| Test | Status |
| --- | --- |
| `testFork_readsRealAaveFlashPremium()` | SKIPPED |
| `testFork_realAaveLoanRepaymentAndWalletPayout()` | SKIPPED |
| `testFork_realV2SushiThenUniswap()` | SKIPPED |
| `testFork_realV2UniswapThenSushi()` | SKIPPED |

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

## Node individual test statuses

| Test | Status |
| --- | --- |
| read-only verifier confirms pinned mainnet wiring but cannot enable trades | PASSED |
| recipient mismatch is a hard verification failure | PASSED |
| deployment missing bytecode fails closed | PASSED |
| wrong chain fails before inspecting contracts | PASSED |
| unverified owner or bytecode cannot be treated as connected | PASSED |
| legacy contract without paused() fails closed | PASSED |
