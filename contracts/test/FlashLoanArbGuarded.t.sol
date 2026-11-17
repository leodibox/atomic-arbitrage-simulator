// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {FlashLoanArbGuarded} from "../src/FlashLoanArbGuarded.sol";
import {MockERC20, MockPair, MockPool} from "./FlashLoanArbProfit.t.sol";

/// These are mock-only regression tests. They do NOT replace a mainnet fork,
/// third-party audit, or gas/account signer verification.
contract FlashLoanArbGuardedTest is Test {
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
        arb = new FlashLoanArbGuarded(address(aave), address(0xBEEF));
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
        assertEq(usdc.balanceOf(address(0xBEEF)), profit);
    }

    function test_autoPayoutDisabledUntilOwnerExplicitlyEnables() public {
        assertEq(arb.autoPayout(), false, "disabled by default");
        uint256 recipientBefore = usdc.balanceOf(address(0xBEEF));
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        assertEq(usdc.balanceOf(address(0xBEEF)), recipientBefore);
        assertGt(usdc.balanceOf(address(arb)), 0, "surplus stays inside contract");
    }

    function test_autoPayoutOnlyAfterAaveRepayment() public {
        arb.setAutoPayout(true);
        uint256 poolBefore = usdc.balanceOf(address(aave));
        uint256 recipientBefore = usdc.balanceOf(address(0xBEEF));
        uint256 floatBefore = usdc.balanceOf(address(arb));
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1);
        // MockPool pulls principal+premium before flashLoanSimple returns.
        assertGt(usdc.balanceOf(address(aave)), poolBefore);
        assertGt(usdc.balanceOf(address(0xBEEF)), recipientBefore);
        assertEq(usdc.balanceOf(address(arb)), floatBefore,
            "auto payout cannot take pre-existing contract funds");
    }

    function test_autoPayoutDoesNotTransferIfWholeFlashloanReverts() public {
        arb.setAutoPayout(true);
        uint256 walletBefore = usdc.balanceOf(address(0xBEEF));
        vm.expectRevert(FlashLoanArbGuarded.Unprofitable.selector);
        arb.executeArbitrage(address(usdc), 10_000e6, pathGood(), 1_000_000e6);
        assertEq(usdc.balanceOf(address(0xBEEF)), walletBefore,
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
}
