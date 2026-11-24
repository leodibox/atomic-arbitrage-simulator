// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {FlashLoanArbGuarded} from "../src/FlashLoanArbGuarded.sol";

interface IERC20Fork {
    function balanceOf(address) external view returns (uint256);
    function allowance(address, address) external view returns (uint256);
}

/// Fork-only synthetic AMM using *real* Ethereum USDT and WETH token contracts.
/// Reserves are fabricated and explicitly NOT historical public liquidity.
/// Models canonical V2 30 bps invariant and accepts no-return USDT transfers.
contract ForkSyntheticV2Pair {
    address public immutable token0;
    address public immutable token1;
    uint112 private reserve0;
    uint112 private reserve1;
    constructor(address a, address b) { token0 = a; token1 = b; }
    function getReserves() external view returns (uint112, uint112, uint32) {
        return (reserve0, reserve1, 0);
    }
    function sync() external {
        reserve0 = uint112(IERC20Fork(token0).balanceOf(address(this)));
        reserve1 = uint112(IERC20Fork(token1).balanceOf(address(this)));
    }
    function _transfer(address asset, address to, uint256 amount) private {
        (bool ok, bytes memory data) = asset.call(
            abi.encodeWithSignature("transfer(address,uint256)", to, amount)
        );
        require(ok && (data.length == 0 || (data.length == 32 && abi.decode(data, (bool)))),
            "pair transfer failed");
    }
    function swap(uint256 out0, uint256 out1, address to, bytes calldata) external {
        require(out0 != 0 || out1 != 0, "zero output");
        uint256 r0 = reserve0;
        uint256 r1 = reserve1;
        require(out0 < r0 && out1 < r1, "insufficient reserve");
        if (out0 != 0) _transfer(token0, to, out0);
        if (out1 != 0) _transfer(token1, to, out1);
        uint256 b0 = IERC20Fork(token0).balanceOf(address(this));
        uint256 b1 = IERC20Fork(token1).balanceOf(address(this));
        uint256 in0 = b0 > r0 - out0 ? b0 - (r0 - out0) : 0;
        uint256 in1 = b1 > r1 - out1 ? b1 - (r1 - out1) : 0;
        require(in0 != 0 || in1 != 0, "no input");
        require((b0 * 1000 - in0 * 3) * (b1 * 1000 - in1 * 3)
            >= r0 * r1 * 1000 * 1000, "invalid V2 invariant");
        reserve0 = uint112(b0);
        reserve1 = uint112(b1);
    }
}

interface IAavePremiumFork {
    function FLASHLOAN_PREMIUM_TOTAL() external view returns (uint128);
}

/// Positive end-to-end integration through real Aave and real USDT (not
/// an observed profitable trade). Synthetic price gap ONLY in local V2 pairs.
/// No mainnet broadcasting, no real wallets, no deployed trading contract.
contract GuardedPositiveUsdtForkTest is Test {
    address constant AAVE = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2;
    address constant USDT = 0xdAC17F958D2ee523a2206206994597C13D831ec7;
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant RECIPIENT = 0x49826Aa476d6388fcdccAD7b3661597986a53132;

    function _fork() private {
        string memory rpc = vm.envOr("HTTPS_URL", string(""));
        if (bytes(rpc).length == 0) vm.skip(true);
        uint256 atBlock = vm.envOr("MAINNET_FORK_BLOCK", uint256(0));
        if (atBlock == 0) vm.createSelectFork(rpc);
        else vm.createSelectFork(rpc, atBlock);
        assertEq(block.chainid, 1);
        assertGt(USDT.code.length, 0);
        assertGt(WETH.code.length, 0);
        assertGt(AAVE.code.length, 0);
    }
    function _aToken(address asset) private view returns (address token) {
        (bool ok, bytes memory data) = AAVE.staticcall(
            abi.encodeWithSignature("getReserveData(address)", asset)
        );
        require(ok && data.length >= 9 * 32, "invalid Aave reserve data");
        assembly { token := mload(add(data, 288)) }
        require(token.code.length != 0, "missing Aave aToken");
    }

    function _setupPairs() private returns (ForkSyntheticV2Pair cheap, ForkSyntheticV2Pair expensive) {
        cheap = new ForkSyntheticV2Pair(WETH, USDT);
        expensive = new ForkSyntheticV2Pair(WETH, USDT);
        // Fork-only fixtures: 500 WETH / 1m USDT versus 250 WETH / 1m USDT.
        // These reserves do not occur on public exchanges at this block.
        deal(WETH, address(cheap), 500 ether);
        deal(USDT, address(cheap), 1_000_000e6);
        deal(WETH, address(expensive), 250 ether);
        deal(USDT, address(expensive), 1_000_000e6);
        cheap.sync();
        expensive.sync();
        assertEq(IERC20Fork(USDT).balanceOf(address(cheap)), 1_000_000e6);
        assertEq(IERC20Fork(WETH).balanceOf(address(expensive)), 250 ether);
    }

    function _positiveLoan(address asset, uint256 amount, bool usdtLoan) private {
        _fork();
        (ForkSyntheticV2Pair cheap, ForkSyntheticV2Pair expensive) = _setupPairs();
        FlashLoanArbGuarded arb = new FlashLoanArbGuarded(AAVE, RECIPIENT);
        arb.setLoanCap(asset, amount);
        arb.setApprovedPair(address(cheap), true);
        arb.setApprovedPair(address(expensive), true);
        arb.setPaused(false);
        assertFalse(arb.autoPayout(), "no automatic wallet payout");

        address aToken = _aToken(asset);
        uint256 reserveBefore = IERC20Fork(asset).balanceOf(aToken);
        uint256 recipientBefore = IERC20Fork(asset).balanceOf(RECIPIENT);
        uint256 contractBefore = IERC20Fork(asset).balanceOf(address(arb));
        uint256 premium = (amount * IAavePremiumFork(AAVE).FLASHLOAN_PREMIUM_TOTAL() + 9999) / 10000;

        bytes memory path = usdtLoan
            ? abi.encodePacked(uint8(0), address(cheap), uint8(0), uint8(0), address(expensive), uint8(1))
            : abi.encodePacked(uint8(0), address(expensive), uint8(1), uint8(0), address(cheap), uint8(0));
        uint256 gasBefore = gasleft();
        arb.executeArbitrage(asset, amount, path, 1);
        uint256 measured = gasBefore - gasleft();
        uint256 profit = IERC20Fork(asset).balanceOf(address(arb)) - contractBefore;

        assertGt(profit, 0, "positive test must produce real token profit");
        assertEq(IERC20Fork(asset).balanceOf(aToken), reserveBefore + premium,
            "Aave was not repaid full principal and premium");
        assertEq(IERC20Fork(asset).allowance(address(arb), AAVE), 0,
            "Aave allowance remains");
        assertEq(IERC20Fork(asset).balanceOf(RECIPIENT), recipientBefore,
            "owner recipient changed without opt-in");
        assertEq(arb.paused(), false);
        emit log_named_uint("fork_block", block.number);
        emit log_named_address("real_loan_token", asset);
        emit log_named_uint("actual_Aave_premium_raw", premium);
        emit log_named_uint("synthetic_AMM_profit_raw", profit);
        emit log_named_uint("fork_local_execution_gas", measured);
        emit log_string("POSITIVE PATH SYNTHETIC AMM RESERVES: NOT AN OBSERVED MARKET OPPORTUNITY");
    }

    function testFork_realUsdtLoanRealAaveRepaymentWithSyntheticSpread() public {
        _positiveLoan(USDT, 10_000e6, true);
    }

    function testFork_realWethLoanViaRealUsdtTransferWithSyntheticSpread() public {
        _positiveLoan(WETH, 0.1 ether, false);
    }
}
