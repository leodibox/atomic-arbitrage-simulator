// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {FlashLoanArbGuarded} from "../src/FlashLoanArbGuarded.sol";
import {MockERC20, MockPair} from "./FlashLoanArbProfit.t.sol";

interface IAavePoolForFork {
    function FLASHLOAN_PREMIUM_TOTAL() external view returns (uint128);
}

interface RealV2Pair {
    function token0() external view returns (address);
    function token1() external view returns (address);
    function factory() external view returns (address);
    function getReserves() external view returns (uint112, uint112, uint32);
}
interface RealV2Factory {
    function getPair(address, address) external view returns (address);
}

/// Requires an explicit ETHEREUM MAINNET fork and an HTTPS_URL.
/// It does not broadcast or authorize mainnet transactions.
/// If HTTPS_URL is absent, forge marks these tests skipped; skipped tests
/// are NOT evidence that Aave integration has passed.
contract AaveMainnetForkTest is Test {
    address constant AAVE_V3_POOL = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2;
    address constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    address constant RECIPIENT = 0x49826Aa476d6388fcdccAD7b3661597986a53132;

    function _fork() private {
        string memory rpc = vm.envOr("HTTPS_URL", string(""));
        if (bytes(rpc).length == 0) {
            vm.skip(true);
        }
        uint256 forkBlock = vm.envOr("MAINNET_FORK_BLOCK", uint256(0));
        if (forkBlock == 0) vm.createSelectFork(rpc);
        else vm.createSelectFork(rpc, forkBlock);
        assertEq(block.chainid, 1, "fork must point to Ethereum Mainnet");
        assertGt(AAVE_V3_POOL.code.length, 0, "Aave Pool code missing");
        assertGt(USDC.code.length, 0, "real USDC contract missing");
    }

    function testFork_readsRealAaveFlashPremium() public {
        _fork();
        uint128 premiumBps = IAavePoolForFork(AAVE_V3_POOL).FLASHLOAN_PREMIUM_TOTAL();
        assertLe(premiumBps, 100, "unexpectedly high flash premium");
        emit log_named_uint("Aave_flash_premium_bps", premiumBps);
        emit log_named_uint("fork_block", block.number);
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
        assertEq(address(arb.pool()), AAVE_V3_POOL, "wrong Aave pool");
        assertEq(arb.profitRecipient(), RECIPIENT, "wrong receiving address");
        arb.setLoanCap(USDC, 20_000e6);
        arb.setApprovedPair(address(cheap), true);
        arb.setApprovedPair(address(expensive), true);
        arb.setAutoPayout(true);
        arb.setPaused(false);

        bytes memory path = abi.encodePacked(
            address(cheap), uint8(1), address(expensive), uint8(0)
        );
        _executeAndCheckPayout(arb, path);
    }

    function _executeAndCheckPayout(FlashLoanArbGuarded arb, bytes memory path) private {
        uint256 amount = 10_000e6;
        uint256 premium = (amount * IAavePoolForFork(AAVE_V3_POOL).FLASHLOAN_PREMIUM_TOTAL() + 5000) / 10000;
        address aToken = _reserveAToken();
        uint256 liquidityBefore = MockERC20(USDC).balanceOf(aToken);
        uint256 walletBefore = MockERC20(USDC).balanceOf(RECIPIENT);
        vm.expectCall(USDC, abi.encodeWithSignature(
            "transferFrom(address,address,uint256)", address(arb), aToken, amount + premium
        ));
        uint256 beforeGas = gasleft();
        arb.executeArbitrage(USDC, amount, path, 1);
        uint256 executionGas = beforeGas - gasleft();
        uint256 out1 = _quote(amount, 1_000_000e6, 500 ether);
        uint256 out2 = _quote(out1, 250 ether, 1_000_000e6);
        uint256 realizedProfit = MockERC20(USDC).balanceOf(RECIPIENT) - walletBefore;
        assertEq(realizedProfit, out2 - amount - premium, "wrong realized payout");
        assertEq(MockERC20(USDC).balanceOf(aToken), liquidityBefore + premium, "Aave liquidity not repaid");
        assertEq(MockERC20(USDC).allowance(address(arb), AAVE_V3_POOL), 0, "repayment allowance remains");
        assertEq(MockERC20(USDC).balanceOf(address(arb)), 0, "executor retained new USDC");
        emit log_named_uint("borrowed_USDC_raw", amount);
        emit log_named_uint("repaid_USDC_raw", amount + premium);
        emit log_named_uint("wallet_profit_USDC_raw", realizedProfit);
        emit log_named_uint("synthetic_AMM_execution_gas_local_warm", executionGas);
    }

    address constant UNI_USDC_WETH = 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc;
    address constant SUSHI_USDC_WETH = 0x397FF1542f962076d0BFE58eA045FfA2d347ACa0;
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;

    function _reserveAToken() private view returns (address token) {
        (bool ok, bytes memory data) = AAVE_V3_POOL.staticcall(
            abi.encodeWithSignature("getReserveData(address)", USDC)
        );
        require(ok && data.length >= 9 * 32, "invalid Aave reserve data");
        assembly { token := mload(add(data, 288)) }
        require(token.code.length > 0, "aToken missing");
    }

    function _quote(uint256 amount, uint256 reserveIn, uint256 reserveOut) private pure returns (uint256) {
        return amount * 997 * reserveOut / (reserveIn * 1000 + amount * 997);
    }

    function testFork_realV2UniswapThenSushi() public {
        _realV2Route(UNI_USDC_WETH, SUSHI_USDC_WETH);
    }

    function testFork_realV2SushiThenUniswap() public {
        _realV2Route(SUSHI_USDC_WETH, UNI_USDC_WETH);
    }

    /// No deal(), mint(), reserve writes or fabricated profitable opportunities.
    /// An unprofitable route must execute both swaps locally and revert atomically.
    struct RealRouteState {
        uint112 r10;
        uint112 r11;
        uint112 r20;
        uint112 r21;
        uint256 amount;
        uint256 out1;
        uint256 out2;
        uint256 premium;
        uint256 walletBefore;
        address aToken;
        uint256 liquidityBefore;
        bool profitable;
        uint256 beforeGas;
        uint256 executionGas;
    }

    function _realV2Route(address first, address second) private {
        _fork();
        RealRouteState memory state;
        RealV2Pair p1 = RealV2Pair(first);
        RealV2Pair p2 = RealV2Pair(second);
        assertEq(p1.token0(), USDC); assertEq(p1.token1(), WETH);
        assertEq(p2.token0(), USDC); assertEq(p2.token1(), WETH);
        assertEq(RealV2Factory(p1.factory()).getPair(USDC, WETH), first);
        assertEq(RealV2Factory(p2.factory()).getPair(USDC, WETH), second);
        assertEq(p1.factory(), first == UNI_USDC_WETH
            ? address(0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f)
            : address(0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac));
        assertEq(p2.factory(), second == UNI_USDC_WETH
            ? address(0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f)
            : address(0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac));
        (state.r10, state.r11,) = p1.getReserves();
        (state.r20, state.r21,) = p2.getReserves();
        assertGt(state.r10, 0); assertGt(state.r11, 0); assertGt(state.r20, 0); assertGt(state.r21, 0);
        state.amount = 100e6;
        state.out1 = _quote(state.amount, state.r10, state.r11);
        state.out2 = _quote(state.out1, state.r21, state.r20);
        state.premium = (state.amount * IAavePoolForFork(AAVE_V3_POOL).FLASHLOAN_PREMIUM_TOTAL() + 5000) / 10000;
        FlashLoanArbGuarded arb = new FlashLoanArbGuarded(AAVE_V3_POOL, RECIPIENT);
        arb.setLoanCap(USDC, state.amount);
        arb.setApprovedPair(first, true); arb.setApprovedPair(second, true);
        arb.setAutoPayout(true); arb.setPaused(false);
        state.walletBefore = MockERC20(USDC).balanceOf(RECIPIENT);
        state.aToken = _reserveAToken();
        state.liquidityBefore = MockERC20(USDC).balanceOf(state.aToken);
        vm.expectCall(first, abi.encodeWithSignature("swap(uint256,uint256,address,bytes)", 0, state.out1, address(arb), bytes("")));
        vm.expectCall(second, abi.encodeWithSignature("swap(uint256,uint256,address,bytes)", state.out2, 0, address(arb), bytes("")));
        bytes memory path = abi.encodePacked(first, uint8(1), second, uint8(0));
        state.profitable = state.out2 >= state.amount + state.premium + 1;
        if (!state.profitable) vm.expectRevert(FlashLoanArbGuarded.Unprofitable.selector);
        state.beforeGas = gasleft();
        arb.executeArbitrage(USDC, state.amount, path, 1);
        state.executionGas = state.beforeGas - gasleft();
        emit log_named_uint("fork_block", block.number);
        emit log_named_address("first_real_pair", first);
        emit log_named_uint("route_input_USDC_raw", state.amount);
        emit log_named_uint("route_output_USDC_raw", state.out2);
        emit log_named_uint("premium_USDC_raw", state.premium);
        emit log_named_uint("execution_gas_local_warm_including_test_call", state.executionGas);
        emit log_named_uint("profitable_at_fork", state.profitable ? 1 : 0);
        if (state.profitable) {
            assertEq(MockERC20(USDC).balanceOf(RECIPIENT) - state.walletBefore, state.out2 - state.amount - state.premium);
            assertEq(MockERC20(USDC).balanceOf(state.aToken), state.liquidityBefore + state.premium);
        } else {
            assertEq(MockERC20(USDC).balanceOf(RECIPIENT), state.walletBefore);
            assertEq(MockERC20(USDC).balanceOf(state.aToken), state.liquidityBefore);
            (uint112 a, uint112 b,) = p1.getReserves();
            assertEq(a, state.r10); assertEq(b, state.r11);
            (a, b,) = p2.getReserves();
            assertEq(a, state.r20); assertEq(b, state.r21);
        }
        assertEq(MockERC20(USDC).balanceOf(address(arb)), 0);
        assertEq(MockERC20(USDC).allowance(address(arb), AAVE_V3_POOL), 0);
    }
}
