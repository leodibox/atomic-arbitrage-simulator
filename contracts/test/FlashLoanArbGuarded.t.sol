// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {FlashLoanArbGuarded} from "../src/FlashLoanArbGuarded.sol";
import {MockERC20, MockPair, MockPool} from "./FlashLoanArbProfit.t.sol";

/// Observe recipient balance at the actual repayment boundary, not only afterward.
contract RepaymentObservingPool {
    address immutable recipient;
    constructor(address wallet) { recipient = wallet; }
    function flashLoanSimple(address receiver, address asset, uint256 amount, bytes calldata params, uint16) external {
        MockERC20 token = MockERC20(asset);
        uint256 beforeWallet = token.balanceOf(recipient);
        uint256 owed = amount + amount * 5 / 10000;
        token.mint(receiver, amount);
        require(FlashLoanArbGuarded(receiver).executeOperation(asset, amount, owed - amount, receiver, params));
        require(token.balanceOf(recipient) == beforeWallet, "payout before repayment");
        require(token.allowance(receiver, address(this)) == owed, "wrong repayment allowance");
        require(token.transferFrom(receiver, address(this), owed), "repay failed");
        require(token.balanceOf(recipient) == beforeWallet, "payout inside repayment");
    }
}

/// These are mock-only regression tests. They do NOT replace a mainnet fork,
/// third-party audit, or gas/account signer verification.
contract FlashLoanArbGuardedTest is Test {
    address internal constant RECIPIENT = 0x49826Aa476d6388fcdccAD7b3661597986a53132;
    MockERC20 usdc;
    MockERC20 weth;
    MockPair cheap;
    MockPair expensive;
    MockPool aave;
    FlashLoanArbGuarded arb;

    function setUp() public {
        usdc = new MockERC20("USD Coin", "USDC");
        weth = new MockERC20("Wrapped ETH", "WETH");
        cheap = new MockPair(address(usdc), address(weth), 1_000_000e6, 500 ether);
        expensive = new MockPair(address(usdc), address(weth), 1_000_000e6, 250 ether);
        usdc.mint(address(cheap), 1_000_000e6);
        weth.mint(address(cheap), 500 ether);
        usdc.mint(address(expensive), 1_000_000e6);
        weth.mint(address(expensive), 250 ether);
        aave = new MockPool();
        arb = new FlashLoanArbGuarded(address(aave), RECIPIENT);
        arb.setLoanCap(address(usdc), 20_000e6);
        arb.setApprovedPair(address(cheap), true);
        arb.setApprovedPair(address(expensive), true);
        arb.setPaused(false);
    }

    function pathGood() internal view returns (bytes memory) {
        return abi.encodePacked(address(cheap), uint8(1), address(expensive), uint8(0));
    }

    function test_profitableApprovedV2PathAndSweep() public {
        uint256 beforeAmount = usdc.balanceOf(address(arb));
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        uint256 profit = usdc.balanceOf(address(arb)) - beforeAmount;
        assertGt(profit, 0, "expect positive asset surplus after premium");
        arb.sweep(address(usdc), profit);
        assertEq(usdc.balanceOf(RECIPIENT), profit);
    }

    function test_autoPayoutDisabledUntilOwnerExplicitlyEnables() public {
        assertEq(arb.autoPayout(), false, "disabled by default");
        uint256 recipientBefore = usdc.balanceOf(RECIPIENT);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        assertEq(usdc.balanceOf(RECIPIENT), recipientBefore);
        assertGt(usdc.balanceOf(address(arb)), 0, "surplus stays inside contract");
    }

    function test_autoPayoutOnlyAfterAaveRepayment() public {
        arb.setAutoPayout(true);
        uint256 poolBefore = usdc.balanceOf(address(aave));
        uint256 recipientBefore = usdc.balanceOf(RECIPIENT);
        uint256 floatBefore = usdc.balanceOf(address(arb));
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        // MockPool pulls principal+premium before flashLoanSimple returns.
        assertGt(usdc.balanceOf(address(aave)), poolBefore);
        assertGt(usdc.balanceOf(RECIPIENT), recipientBefore);
        assertEq(usdc.balanceOf(address(arb)), floatBefore,
            "auto payout cannot take pre-existing contract funds");
    }

    function test_autoPayoutDoesNotTransferIfWholeFlashloanReverts() public {
        arb.setAutoPayout(true);
        uint256 walletBefore = usdc.balanceOf(RECIPIENT);
        vm.expectRevert(FlashLoanArbGuarded.Unprofitable.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1_000_000e6);
        assertEq(usdc.balanceOf(RECIPIENT), walletBefore,
            "failed atomic attempt must not credit recipient");
    }

    function test_onlyOwnerCanSetAutoPayout() public {
        vm.prank(address(0xBADC0DE));
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        arb.setAutoPayout(true);
        assertEq(arb.autoPayout(), false);
        arb.setAutoPayout(true);
        assertEq(arb.autoPayout(), true);
        arb.setAutoPayout(false);
        assertEq(arb.autoPayout(), false);
    }

    function test_pausedByDefaultAndEmergencyPause() public {
        arb.setPaused(true);
        vm.expectRevert(FlashLoanArbGuarded.Paused.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        arb.setPaused(false);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
    }

    function test_rejectUnapprovedPair() public {
        arb.setApprovedPair(address(expensive), false);
        vm.expectRevert(FlashLoanArbGuarded.UnapprovedPair.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
    }

    function test_rejectWrongDirection() public {
        bytes memory wrong = abi.encodePacked(address(cheap), uint8(0), address(expensive), uint8(1));
        vm.expectRevert(FlashLoanArbGuarded.BadPath.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, wrong, 1);
    }

    function test_rejectInvalidDirectionByte() public {
        bytes memory wrong = abi.encodePacked(address(cheap), uint8(2), address(expensive), uint8(0));
        vm.expectRevert(FlashLoanArbGuarded.BadPath.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, wrong, 1);
    }

    function test_rejectDuplicatePair() public {
        bytes memory wrong = abi.encodePacked(address(cheap), uint8(1), address(cheap), uint8(0));
        vm.expectRevert(FlashLoanArbGuarded.BadPath.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, wrong, 1);
    }

    function test_rejectMissingMinProfit() public {
        vm.expectRevert(FlashLoanArbGuarded.BadPath.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 0);
    }

    function test_rejectOverLoanCap() public {
        vm.expectRevert(FlashLoanArbGuarded.LoanCapExceeded.selector);
        arb.executeArbitrage(address(usdc), 50_000e6, pathGood(), 1);
    }

    function test_rejectInsufficientProfit() public {
        vm.expectRevert(FlashLoanArbGuarded.Unprofitable.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1_000_000e6);
    }

    function test_nonOwnerCannotUnpauseOrAuthorizePair() public {
        vm.startPrank(address(0xBADC0DE));
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        arb.setPaused(true);
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        arb.setApprovedPair(address(cheap), false);
        vm.stopPrank();
    }

    function test_directCallbackRejected() public {
        vm.expectRevert(FlashLoanArbGuarded.NotPool.selector);
        arb.executeOperation(address(usdc), 10_000e6, 0, address(arb), "");
    }

    function test_walletRemainsUnpaidUntilPoolFinishesRepayment() public {
        RepaymentObservingPool observer = new RepaymentObservingPool(RECIPIENT);
        FlashLoanArbGuarded receiver = new FlashLoanArbGuarded(address(observer), RECIPIENT);
        receiver.setLoanCap(address(usdc), 20_000e6);
        receiver.setApprovedPair(address(cheap), true);
        receiver.setApprovedPair(address(expensive), true);
        receiver.setAutoPayout(true);
        receiver.setPaused(false);
        receiver.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        assertEq(usdc.balanceOf(address(observer)), 10_005e6);
        assertEq(usdc.balanceOf(RECIPIENT), expectedProfit(10_000e6));
    }

    function expectedProfit(uint256 amount) internal pure returns (uint256) {
        uint256 wethOut = amount * 997 * 500 ether / (1_000_000e6 * 1000 + amount * 997);
        uint256 usdcOut = wethOut * 997 * 1_000_000e6 / (250 ether * 1000 + wethOut * 997);
        return usdcOut - amount - amount * 5 / 10000;
    }

    function test_exactPrincipalPremiumAndConfirmedWalletPayout() public {
        uint256 amount = 10_000e6;
        uint256 profit = expectedProfit(amount);
        usdc.mint(address(arb), 123e6);
        arb.setAutoPayout(true);
        vm.expectCall(address(usdc), abi.encodeWithSignature(
            "transferFrom(address,address,uint256)", address(arb), address(aave), amount + amount * 5 / 10000
        ));
        arb.executeArbitrage(address(usdc), amount, pathGood(), profit);
        assertEq(arb.profitRecipient(), RECIPIENT);
        assertEq(usdc.balanceOf(RECIPIENT), profit);
        assertEq(usdc.balanceOf(address(aave)), amount + amount * 5 / 10000);
        assertEq(usdc.balanceOf(address(arb)), 123e6, "pre-existing funds preserved");
        assertEq(usdc.allowance(address(arb), address(aave)), 0, "repayment allowance exhausted");
    }

    function test_minProfitAboveNetAfterPremiumRevertsAtomically() public {
        arb.setAutoPayout(true);
        vm.expectRevert(FlashLoanArbGuarded.Unprofitable.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), expectedProfit(10_000e6) + 1);
        assertRolledBack();
    }

    function test_failedAaveRepaymentNeverPaysWallet() public {
        arb.setAutoPayout(true);
        vm.mockCall(address(usdc), abi.encodeWithSignature(
            "transferFrom(address,address,uint256)", address(arb), address(aave), uint256(10_005e6)
        ), abi.encode(false));
        vm.expectRevert(bytes("repay failed"));
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        assertRolledBack();
    }

    function test_failedWalletTransferRollsBackLoanAndSwaps() public {
        arb.setAutoPayout(true);
        vm.mockCall(address(usdc), abi.encodeWithSignature("transfer(address,uint256)", RECIPIENT), abi.encode(false));
        vm.expectRevert(FlashLoanArbGuarded.TransferFailed.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        assertRolledBack();
    }

    function test_failedInputTransferRollsBack() public {
        vm.mockCall(address(usdc), abi.encodeWithSignature("transfer(address,uint256)", address(cheap)), abi.encode(false));
        vm.expectRevert(FlashLoanArbGuarded.TransferFailed.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        assertRolledBack();
    }

    function test_failedApprovalRollsBack() public {
        vm.mockCall(address(usdc), abi.encodeWithSignature("approve(address,uint256)", address(aave)), abi.encode(false));
        vm.expectRevert(FlashLoanArbGuarded.TransferFailed.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        assertRolledBack();
    }

    function test_emptyReturnApprovalSupported() public {
        // Initial allowance is already zero; no-return zero-reset is supported.
        vm.mockCall(address(usdc), abi.encodeWithSignature("approve(address,uint256)", address(aave), uint256(0)), bytes(""));
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        assertEq(usdc.allowance(address(arb), address(aave)), 0);
        assertGt(usdc.balanceOf(address(arb)), 0);
    }

    function test_poolCannotCallOutsideActiveLoan() public {
        vm.prank(address(aave));
        vm.expectRevert(FlashLoanArbGuarded.Reentrant.selector);
        arb.executeOperation(address(usdc), 10_000e6, 5e6, address(arb), abi.encode(pathGood(), 1));
    }

    function test_nonOwnerCannotExecuteSweepOrSetLoanCap() public {
        vm.startPrank(address(0xBAD));
        // Trade access is owner OR explicitly-approved operator; strangers
        // must now fail with NotExecutor, while treasury/admin remain NotOwner.
        vm.expectRevert(FlashLoanArbGuarded.NotExecutor.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        arb.sweep(address(usdc), 1);
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        arb.setLoanCap(address(usdc), 1);
        vm.stopPrank();
    }

    function test_constructorPausedAndDisabledByDefault() public {
        FlashLoanArbGuarded fresh = new FlashLoanArbGuarded(address(aave), RECIPIENT);
        assertTrue(fresh.paused());
        assertFalse(fresh.autoPayout());
        assertEq(fresh.maxLoanRaw(address(usdc)), 0);
        assertFalse(fresh.approvedPair(address(cheap)));
    }

    function test_zeroAmountAndDisabledAssetRejected() public {
        vm.expectRevert(FlashLoanArbGuarded.LoanCapExceeded.selector);
        arb.executeArbitrage(address(usdc), 0, pathGood(), 1);
        arb.setLoanCap(address(usdc), 0);
        vm.expectRevert(FlashLoanArbGuarded.LoanCapExceeded.selector);
        arb.executeArbitrage(address(usdc), 1e6, pathGood(), 1);
    }

    function testFuzz_payoutPreservesFloat(uint64 rawAmount, uint64 priorFloat) public {
        uint256 amount = bound(uint256(rawAmount), 1e6, 20_000e6);
        usdc.mint(address(arb), uint256(priorFloat));
        arb.setAutoPayout(true);
        uint256 profit = expectedProfit(amount);
        arb.executeArbitrage(address(usdc), amount, pathGood(), profit);
        assertEq(usdc.balanceOf(RECIPIENT), profit);
        assertEq(usdc.balanceOf(address(arb)), uint256(priorFloat));
        assertEq(usdc.balanceOf(address(aave)), amount + amount * 5 / 10000);
        assertEq(usdc.allowance(address(arb), address(aave)), 0);
    }

    function assertRolledBack() internal view {
        assertEq(usdc.balanceOf(RECIPIENT), 0);
        assertEq(usdc.balanceOf(address(aave)), 0);
        assertEq(usdc.balanceOf(address(arb)), 0);
        assertEq(usdc.allowance(address(arb), address(aave)), 0);
        assertEq(usdc.balanceOf(address(cheap)), 1_000_000e6);
        assertEq(usdc.balanceOf(address(expensive)), 1_000_000e6);
        assertEq(weth.balanceOf(address(cheap)), 500 ether);
        assertEq(weth.balanceOf(address(expensive)), 250 ether);
        (uint112 c0, uint112 c1,) = cheap.getReserves();
        (uint112 e0, uint112 e1,) = expensive.getReserves();
        assertEq(c0, 1_000_000e6); assertEq(c1, 500 ether);
        assertEq(e0, 1_000_000e6); assertEq(e1, 250 ether);
    }
}
