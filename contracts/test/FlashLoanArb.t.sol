// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {FlashLoanArb} from "../src/FlashLoanArb.sol";

interface IERC20 {
    function balanceOf(address) external view returns (uint256);
    function transfer(address, uint256) external returns (bool);
    function approve(address, uint256) external returns (bool);
}

interface IPair {
    function getReserves() external view returns (uint112, uint112, uint32);
    function swap(uint256, uint256, address, bytes calldata) external;
    function token0() external view returns (address);
    function token1() external view returns (address);
}

contract FlashLoanArbTest is Test {
    // Aave V3 mainnet
    address constant POOL = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2;
    address constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    address constant PROFIT_RECIPIENT = 0x49826Aa476d6388fcdccAD7b3661597986a53132;
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    // Uniswap V2 USDC/WETH
    address constant UNI_PAIR = 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc;
    // SushiSwap USDC/WETH
    address constant SUSHI_PAIR = 0x397FF1542f962076d0BFE58eA045FfA2d347ACa0;

    FlashLoanArb arb;
    address owner;

    function setUp() public {
        vm.createSelectFork(vm.envString("HTTPS_URL"));
        owner = address(this);
        arb = new FlashLoanArb(POOL, PROFIT_RECIPIENT);
    }

    function _path(address p1, uint8 d1, address p2, uint8 d2) internal pure returns (bytes memory) {
        return abi.encodePacked(p1, d1, p2, d2);
    }

    /// Successful repayment: real mainnet pool, real gap block. Use a historical
    /// block where a price gap existed; here we just verify the callback plumbing
    /// by checking the contract reverts cleanly on unprofitable (no gap) paths.
    function test_unprofitableReverts() public {
        // No price gap at this block → arb loses money → Unprofitable revert.
        vm.expectRevert(FlashLoanArb.Unprofitable.selector);
        arb.executeArbitrage(USDC, 10_000e6, _path(UNI_PAIR, 1, SUSHI_PAIR, 0), 1);
    }

    function test_unauthorizedCallerReverts() public {
        vm.prank(address(0xBEEF));
        vm.expectRevert(FlashLoanArb.NotOwner.selector);
        arb.executeArbitrage(USDC, 1_000e6, _path(UNI_PAIR, 1, SUSHI_PAIR, 0), 1);
    }

    function test_unauthorizedExecuteOperationReverts() public {
        vm.prank(address(0xBEEF));
        vm.expectRevert(FlashLoanArb.NotPool.selector);
        arb.executeOperation(USDC, 1_000e6, 5e4, address(arb), abi.encode(_path(UNI_PAIR, 1, SUSHI_PAIR, 0), uint256(1)));
    }

    function test_badPathReverts() public {
        vm.expectRevert(FlashLoanArb.BadPath.selector);
        arb.executeArbitrage(USDC, 1_000e6, hex"deadbeef", 1);
    }

    function test_ownerIsDeployer() public view {
        assertEq(arb.owner(), owner);
    }

    function test_poolAddress() public view {
        assertEq(address(arb.pool()), POOL);
    }

    function test_profitRecipient() public view {
        assertEq(arb.profitRecipient(), PROFIT_RECIPIENT);
    }

    function test_sweepUnauthorizedReverts() public {
        vm.prank(address(0xBEEF));
        vm.expectRevert(FlashLoanArb.NotOwner.selector);
        arb.sweep(USDC, 1);
    }

    function test_zeroProfitRecipientReverts() public {
        vm.expectRevert(FlashLoanArb.ZeroAddress.selector);
        new FlashLoanArb(POOL, address(0));
    }
}
