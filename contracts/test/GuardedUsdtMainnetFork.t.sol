// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {FlashLoanArbGuarded} from "../src/FlashLoanArbGuarded.sol";

interface IUsdtForkPair {
    function token0() external view returns (address);
    function token1() external view returns (address);
    function factory() external view returns (address);
    function getReserves() external view returns (uint112, uint112, uint32);
}

interface IUsdtForkFactory {
    function getPair(address, address) external view returns (address);
}

interface IUsdtForkErc20 {
    function balanceOf(address) external view returns (uint256);
    function allowance(address, address) external view returns (uint256);
}

interface IUsdtForkAave {
    function FLASHLOAN_PREMIUM_TOTAL() external view returns (uint128);
}

/// Real Aave V3 pool and real Uniswap V2 / SushiSwap WETH-USDT pools on
/// ETHEREUM MAINNET FORK. Never broadcasts or fabricates liquidity.
/// Test negative routes must revert atomically; profitable routes (if any at
/// a particular block) must repay Aave and retain the net asset surplus.
/// Not proof of positive net USD profit: actual ETH gas is excluded.
contract GuardedUsdtMainnetForkTest is Test {
    address constant AAVE = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2;
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant USDT = 0xdAC17F958D2ee523a2206206994597C13D831ec7;
    address constant UNI = 0x0d4a11d5EEaaC28EC3F61d100daF4d40471f1852;
    address constant SUSHI = 0x06da0fd433C1A5d7a4faa01111c044910A184553;
    address constant UNI_FACTORY = 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f;
    address constant SUSHI_FACTORY = 0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac;
    address constant RECIPIENT = 0x49826Aa476d6388fcdccAD7b3661597986a53132;

    struct Route {
        uint256 amount;
        uint256 premium;
        uint256 out1;
        uint256 out2;
        uint256 beforeGas;
        uint256 afterGas;
        uint256 executorBefore;
        uint256 recipientBefore;
        uint256 aTokenBefore;
        address aToken;
        uint112 p10;
        uint112 p11;
        uint112 p20;
        uint112 p21;
        uint8 direction1;
        uint8 direction2;
        bool profitableBeforeGas;
    }

    function _fork() private {
        string memory rpc = vm.envOr("HTTPS_URL", string(""));
        if (bytes(rpc).length == 0) vm.skip(true);
        uint256 pinnedBlock = vm.envOr("MAINNET_FORK_BLOCK", uint256(0));
        if (pinnedBlock == 0) vm.createSelectFork(rpc);
        else vm.createSelectFork(rpc, pinnedBlock);
        assertEq(block.chainid, 1, "not Ethereum mainnet");
        assertGt(AAVE.code.length, 0, "missing real Aave pool");
        assertGt(USDT.code.length, 0, "missing real USDT");
        assertGt(WETH.code.length, 0, "missing real WETH");
    }

    function _aToken(address asset) private view returns (address token) {
        (bool ok, bytes memory data) = AAVE.staticcall(
            abi.encodeWithSignature("getReserveData(address)", asset)
        );
        require(ok && data.length >= 9 * 32, "missing Aave reserve data");
        assembly { token := mload(add(data, 288)) }
        require(token.code.length != 0, "missing aToken");
    }

    function _quote(uint256 amount, uint256 reserveIn, uint256 reserveOut)
        private pure returns (uint256)
    {
        return amount * 997 * reserveOut / (reserveIn * 1000 + amount * 997);
    }

    function _validate(address first, address second) private view {
        IUsdtForkPair p1 = IUsdtForkPair(first);
        IUsdtForkPair p2 = IUsdtForkPair(second);
        assertEq(p1.factory(), first == UNI ? UNI_FACTORY : SUSHI_FACTORY);
        assertEq(p2.factory(), second == UNI ? UNI_FACTORY : SUSHI_FACTORY);
        assertEq(IUsdtForkFactory(p1.factory()).getPair(WETH, USDT), first);
        assertEq(IUsdtForkFactory(p2.factory()).getPair(WETH, USDT), second);
        assertEq(p1.token0(), WETH, "unexpected token0 WETH");
        assertEq(p1.token1(), USDT, "unexpected token1 USDT");
        assertEq(p2.token0(), WETH, "unexpected token0 WETH");
        assertEq(p2.token1(), USDT, "unexpected token1 USDT");
    }

    function _route(address asset, address first, address second, uint256 amount) private {
        _fork();
        _validate(first, second);
        Route memory s;
        s.amount = amount;
        s.direction1 = asset == WETH ? 1 : 0;
        s.direction2 = asset == WETH ? 0 : 1;
        (s.p10, s.p11,) = IUsdtForkPair(first).getReserves();
        (s.p20, s.p21,) = IUsdtForkPair(second).getReserves();
        assertGt(s.p10, 0); assertGt(s.p11, 0);
        assertGt(s.p20, 0); assertGt(s.p21, 0);
        s.out1 = s.direction1 == 1
            ? _quote(amount, s.p10, s.p11)
            : _quote(amount, s.p11, s.p10);
        s.out2 = s.direction2 == 1
            ? _quote(s.out1, s.p20, s.p21)
            : _quote(s.out1, s.p21, s.p20);
        s.premium = (amount * IUsdtForkAave(AAVE).FLASHLOAN_PREMIUM_TOTAL() + 9999) / 10000;
        s.profitableBeforeGas = s.out2 > s.amount + s.premium;

        FlashLoanArbGuarded arb = new FlashLoanArbGuarded(AAVE, RECIPIENT);
        arb.setLoanCap(asset, amount);
        arb.setApprovedPair(first, true);
        arb.setApprovedPair(second, true);
        arb.setPaused(false);
        // Payout stays disabled; actual surplus must remain owner-controlled.
        assertFalse(arb.autoPayout());
        s.executorBefore = IUsdtForkErc20(asset).balanceOf(address(arb));
        s.recipientBefore = IUsdtForkErc20(asset).balanceOf(RECIPIENT);
        s.aToken = _aToken(asset);
        s.aTokenBefore = IUsdtForkErc20(asset).balanceOf(s.aToken);
        bytes memory path = abi.encodePacked(first, s.direction1, second, s.direction2);
        if (!s.profitableBeforeGas)
            vm.expectRevert(FlashLoanArbGuarded.Unprofitable.selector);
        s.beforeGas = gasleft();
        arb.executeArbitrage(asset, amount, path, 1);
        s.afterGas = gasleft();

        emit log_named_uint("fork_block", block.number);
        emit log_named_address("start_asset", asset);
        emit log_named_address("first_pair", first);
        emit log_named_address("second_pair", second);
        emit log_named_uint("input_raw", amount);
        emit log_named_uint("quoted_output_raw", s.out2);
        emit log_named_uint("aave_premium_raw", s.premium);
        emit log_named_uint("gross_profitable_after_premium_before_gas", s.profitableBeforeGas ? 1 : 0);
        emit log_named_uint("local_execution_gas_including_test_call", s.beforeGas - s.afterGas);

        assertEq(IUsdtForkErc20(asset).balanceOf(RECIPIENT), s.recipientBefore,
            "recipient changed without owner payout");
        if (s.profitableBeforeGas) {
            assertEq(IUsdtForkErc20(asset).balanceOf(s.aToken),
                s.aTokenBefore + s.premium, "real Aave repayment failed");
            assertEq(IUsdtForkErc20(asset).balanceOf(address(arb)),
                s.executorBefore + s.out2 - s.amount - s.premium,
                "wrong onchain surplus");
        } else {
            assertEq(IUsdtForkErc20(asset).balanceOf(s.aToken), s.aTokenBefore,
                "reverted loan changed reserve liquidity");
            assertEq(IUsdtForkErc20(asset).balanceOf(address(arb)), s.executorBefore);
            (uint112 x0, uint112 x1,) = IUsdtForkPair(first).getReserves();
            assertEq(x0, s.p10); assertEq(x1, s.p11);
            (x0, x1,) = IUsdtForkPair(second).getReserves();
            assertEq(x0, s.p20); assertEq(x1, s.p21);
        }
        assertEq(IUsdtForkErc20(asset).allowance(address(arb), AAVE), 0);
    }

    /// Directly verify nonstandard real USDT approve return bytes on a fork,
    /// without funding any wallet or authorizing a real transaction.
    function testFork_realUsdtApproveZeroAndNonzeroReturnsEmpty() public {
        _fork();
        (bool ok, bytes memory ret) = USDT.call(
            abi.encodeWithSignature("approve(address,uint256)", AAVE, 0)
        );
        assertTrue(ok, "USDT approve(0) failed");
        assertEq(ret.length, 0, "unexpected USDT approval returndata");
        (ok, ret) = USDT.call(
            abi.encodeWithSignature("approve(address,uint256)", AAVE, 123456)
        );
        assertTrue(ok, "USDT approve(nonzero) failed");
        assertEq(ret.length, 0, "unexpected USDT approval returndata");
        assertEq(IUsdtForkErc20(USDT).allowance(address(this), AAVE), 123456);
        (ok, ret) = USDT.call(
            abi.encodeWithSignature("approve(address,uint256)", AAVE, 0)
        );
        assertTrue(ok, "USDT allowance reset failed");
        assertEq(ret.length, 0);
        assertEq(IUsdtForkErc20(USDT).allowance(address(this), AAVE), 0);
        emit log_named_uint("fork_block", block.number);
    }

    function testFork_WethUniThenSushiUsdtTransfer() public {
        _route(WETH, UNI, SUSHI, 0.05 ether);
    }

    function testFork_WethSushiThenUniUsdtTransfer() public {
        _route(WETH, SUSHI, UNI, 0.05 ether);
    }

    function testFork_UsdtUniThenSushiFlashLoan() public {
        _route(USDT, UNI, SUSHI, 100e6);
    }

    function testFork_UsdtSushiThenUniFlashLoan() public {
        _route(USDT, SUSHI, UNI, 100e6);
    }
}
