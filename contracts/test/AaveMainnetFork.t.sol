// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {FlashLoanArbGuarded} from "../src/FlashLoanArbGuarded.sol";
import {MockERC20, MockPair} from "./FlashLoanArbProfit.t.sol";

interface IAavePoolForFork {
    function FLASHLOAN_PREMIUM_TOTAL() external view returns (uint128);
}

/// Requires an explicit ETHEREUM MAINNET fork and a private HTTPS_URL.
/// It does not broadcast or authorize mainnet transactions.
/// If HTTPS_URL is absent, forge marks these tests skipped; skipped tests
/// are NOT evidence that Aave integration has passed.
contract AaveMainnetForkTest is Test {
    address constant AAVE_V3_POOL = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2;
    address constant USDC = 0xA0b86991c6218b36c1d19D4a2e9eB0cE3606eB48;
    address constant RECIPIENT = 0x49826Aa476d6388fcdccAD7b3661597986a53132;

    function _fork() private {
        string memory rpc = vm.envOr("HTTPS_URL", string(""));
        if (bytes(rpc).length == 0) {
            vm.skip(true);
        }
        vm.createSelectFork(rpc);
        assertEq(block.chainid, 1, "fork must point to Ethereum Mainnet");
        assertGt(AAVE_V3_POOL.code.length, 0, "Aave Pool code missing");
        assertGt(USDC.code.length, 0, "real USDC contract missing");
    }

    function testFork_readsRealAaveFlashPremium() public {
        _fork();
        uint128 premiumBps = IAavePoolForFork(AAVE_V3_POOL).FLASHLOAN_PREMIUM_TOTAL();
        assertLe(premiumBps, 100, "unexpectedly high flash premium");
    }

    /// Test a real Aave V3 Pool flash loan and repayment on a MAINNET FORK,
    /// using mocked V2 pairs. No real arbitrage or production pair is proven.
    function testFork_realAaveLoanRepaymentAndWalletPayout() public {
        _fork();

        MockERC20 mockWeth = new MockERC20("Mock WETH", "mWETH");
        MockPair cheap = new MockPair(USDC, address(mockWeth), 1_000_000e6, 500 ether);
        MockPair expensive = new MockPair(USDC, address(mockWeth), 1_000_000e6, 250 ether);

        // Set up synthetic on-fork AMM reserves, not public exchange liquidity.
        deal(USDC, address(cheap), 1_000_000e6);
        deal(USDC, address(expensive), 1_000_000e6);
        mockWeth.mint(address(cheap), 500 ether);
        mockWeth.mint(address(expensive), 250 ether);

        FlashLoanArbGuarded arb = new FlashLoanArbGuarded(AAVE_V3_POOL, RECIPIENT);
        assertEq(arb.pool(), AAVE_V3_POOL, "wrong Aave pool");
        assertEq(arb.profitRecipient(), RECIPIENT, "wrong receiving address");
        arb.setLoanCap(USDC, 20_000e6);
        arb.setApprovedPair(address(cheap), true);
        arb.setApprovedPair(address(expensive), true);
        arb.setAutoPayout(true);
        arb.setPaused(false);

        bytes memory path = abi.encodePacked(
            address(cheap), uint8(1), address(expensive), uint8(0)
        );
        uint256 recipientBefore = MockERC20(USDC).balanceOf(RECIPIENT);
        arb.executeArbitrage(USDC, 10_000e6, path, 1);

        uint256 recipientAfter = MockERC20(USDC).balanceOf(RECIPIENT);
        assertGt(recipientAfter, recipientBefore, "actual Aave callback did not pay USDC");
        assertEq(MockERC20(USDC).balanceOf(address(arb)), 0,
            "successful payout should not leave new USDC in executor");
    }
}
