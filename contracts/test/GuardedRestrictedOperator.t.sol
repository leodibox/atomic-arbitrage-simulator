// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {FlashLoanArbGuarded} from "../src/FlashLoanArbGuarded.sol";
import {MockERC20, MockPair, MockPool} from "./FlashLoanArbProfit.t.sol";

contract GuardedRestrictedOperatorTest is Test {
    address constant RECIPIENT = 0x49826Aa476d6388fcdccAD7b3661597986a53132;
    address constant OPERATOR = address(0xB0B);
    address constant OUTSIDER = address(0xCAFE);

    MockERC20 usdc;
    MockERC20 weth;
    MockPair cheap;
    MockPair expensive;
    MockPool aave;
    FlashLoanArbGuarded guarded;

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
        guarded = new FlashLoanArbGuarded(address(aave), RECIPIENT);
        guarded.setLoanCap(address(usdc), 20_000e6);
        guarded.setApprovedPair(address(cheap), true);
        guarded.setApprovedPair(address(expensive), true);
        guarded.setPaused(false);
    }

    function path() internal view returns (bytes memory) {
        return abi.encodePacked(address(cheap), uint8(1), address(expensive), uint8(0));
    }

    function testOperatorDeniedByDefault() public {
        assertFalse(guarded.approvedOperator(OPERATOR));
        vm.expectRevert(FlashLoanArbGuarded.NotExecutor.selector);
        vm.prank(OPERATOR);
        guarded.executeArbitrage(address(usdc), 10_000e6, path(), 1);
    }

    function testAuthorizedOperatorExecutesAndProfitStaysInContract() public {
        guarded.setApprovedOperator(OPERATOR, true);
        assertTrue(guarded.approvedOperator(OPERATOR));
        uint256 walletBefore = usdc.balanceOf(RECIPIENT);
        vm.prank(OPERATOR);
        guarded.executeArbitrage(address(usdc), 10_000e6, path(), 1);
        assertEq(usdc.balanceOf(RECIPIENT), walletBefore);
        assertGt(usdc.balanceOf(address(guarded)), 0);
        // Only the wallet that deployed the contract can collect surplus.
        uint256 surplus = usdc.balanceOf(address(guarded));
        guarded.sweep(address(usdc), surplus);
        assertEq(usdc.balanceOf(RECIPIENT), walletBefore + surplus);
    }

    function testOperatorCannotModifyControlsOrWithdraw() public {
        guarded.setApprovedOperator(OPERATOR, true);
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        vm.prank(OPERATOR);
        guarded.setPaused(true);
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        vm.prank(OPERATOR);
        guarded.setLoanCap(address(usdc), 1);
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        vm.prank(OPERATOR);
        guarded.setApprovedPair(address(cheap), false);
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        vm.prank(OPERATOR);
        guarded.setAutoPayout(true);
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        vm.prank(OPERATOR);
        guarded.sweep(address(usdc), 1);
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        vm.prank(OPERATOR);
        guarded.setApprovedOperator(OUTSIDER, true);
    }

    function testOwnerCanRevokeOperatorAndPause() public {
        guarded.setApprovedOperator(OPERATOR, true);
        guarded.setPaused(true);
        vm.expectRevert(FlashLoanArbGuarded.Paused.selector);
        vm.prank(OPERATOR);
        guarded.executeArbitrage(address(usdc), 10_000e6, path(), 1);
        guarded.setPaused(false);
        guarded.setApprovedOperator(OPERATOR, false);
        vm.expectRevert(FlashLoanArbGuarded.NotExecutor.selector);
        vm.prank(OPERATOR);
        guarded.executeArbitrage(address(usdc), 10_000e6, path(), 1);
    }

    function testStrangerCannotAuthorizeAndZeroOperatorRejected() public {
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        vm.prank(OUTSIDER);
        guarded.setApprovedOperator(OUTSIDER, true);
        vm.expectRevert(FlashLoanArbGuarded.ZeroAddress.selector);
        guarded.setApprovedOperator(address(0), true);
        vm.expectRevert(FlashLoanArbGuarded.ZeroAddress.selector);
        guarded.setApprovedOperator(address(guarded), true);
    }
}
