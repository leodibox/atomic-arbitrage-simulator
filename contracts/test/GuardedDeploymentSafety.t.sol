// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {FlashLoanArbGuarded} from "../src/FlashLoanArbGuarded.sol";

/// @notice Deployment identity and safe defaults only. No blockchain broadcast.
contract GuardedDeploymentSafetyTest is Test {
    address constant EXPECTED_OWNER = 0x49826Aa476d6388fcdccAD7b3661597986a53132;
    address constant AAVE_V3_POOL = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2;

    function _deployAsWallet() internal returns (FlashLoanArbGuarded guarded) {
        // The real Pool is checked on a separately reviewed mainnet fork.
        // Here a minimal nonempty bytecode avoids network or Aave dependencies.
        vm.etch(AAVE_V3_POOL, hex"00");
        vm.prank(EXPECTED_OWNER);
        guarded = new FlashLoanArbGuarded(AAVE_V3_POOL, EXPECTED_OWNER);
    }

    function testWalletDeploySetsBothOwnerAndRecipient() public {
        FlashLoanArbGuarded guarded = _deployAsWallet();
        assertEq(guarded.owner(), EXPECTED_OWNER);
        assertEq(guarded.profitRecipient(), EXPECTED_OWNER);
        assertEq(address(guarded.pool()), AAVE_V3_POOL);
        assertTrue(guarded.paused());
        assertFalse(guarded.autoPayout());
    }

    function testDeploymentRemainsUnarmedAndCannotBeUnpausedByStranger() public {
        FlashLoanArbGuarded guarded = _deployAsWallet();
        address outsider = address(0xBEEF);
        vm.prank(outsider);
        vm.expectRevert(FlashLoanArbGuarded.NotOwner.selector);
        guarded.setPaused(false);
        assertTrue(guarded.paused());
        assertEq(guarded.maxLoanRaw(address(0xCAFE)), 0);
        assertFalse(guarded.approvedPair(address(0xCAFE)));
    }

    function testOnlyOwnerCanChangePauseAfterDeliberateApproval() public {
        FlashLoanArbGuarded guarded = _deployAsWallet();
        vm.prank(EXPECTED_OWNER);
        guarded.setPaused(false);
        assertFalse(guarded.paused());
        vm.prank(EXPECTED_OWNER);
        guarded.setPaused(true);
        assertTrue(guarded.paused());
    }
}
