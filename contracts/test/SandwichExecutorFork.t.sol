// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {SandwichExecutor} from "../src/SandwichExecutor.sol";

interface IERC20Fork {
    function balanceOf(address) external view returns (uint256);
    function approve(address, uint256) external returns (bool);
}

interface IV2RouterFork {
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory);
}

interface IV2FactoryFork {
    function getPair(address, address) external view returns (address);
}

interface IV2PairFork {
    function token0() external view returns (address);
    function getReserves() external view returns (uint112, uint112, uint32);
}

/// End-to-end sandwich on real mainnet state: the executor opens, a real victim
/// swap lands through the V2 router, and the executor closes. The pool is small
/// by a 300 WETH victim, whose 6% impact beats the two 0.3% fees.
/// Skips when HTTPS_URL is missing, so CI without secrets stays green.
contract SandwichExecutorForkTest is Test {
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    address constant ROUTER = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
    address constant FACTORY = 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f;
    address constant PAIR = 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc;

    uint256 constant FLOAT = 3 ether;
    uint256 constant FRONT = 0.5 ether;
    uint256 constant VICTIM_IN = 300 ether;

    SandwichExecutor exec;
    address victim = address(0xBEEF);

    function setUp() public {
        string memory rpc = vm.envOr("HTTPS_URL", string(""));
        if (bytes(rpc).length == 0) vm.skip(true);
        uint256 n = vm.envOr("MAINNET_FORK_BLOCK", uint256(0));
        if (n == 0) vm.createSelectFork(rpc);
        else vm.createSelectFork(rpc, n);
        assertEq(block.chainid, 1);

        exec = new SandwichExecutor();
        deal(WETH, address(exec), FLOAT);
        deal(WETH, victim, VICTIM_IN);
    }
    function _steps(uint8 dir, address tokenIn, uint256 amountIn)
        private
        view
        returns (bytes memory)
    {
        return abi.encodePacked(uint64(block.number), dir, PAIR, tokenIn, amountIn, uint256(0));
    }

    function _exec(bytes memory steps, uint256 minProfit) private returns (bool ok) {
        bytes memory cd = abi.encodePacked(
            abi.encodeWithSelector(SandwichExecutor.execute.selector, minProfit),
            steps
        );
        (ok, ) = address(exec).call(cd);
    }

    function testRealVictimCyclePaysTheOwner() public {
        assertEq(IV2PairFork(PAIR).token0(), USDC, "pair layout changed");
        assertEq(IV2FactoryFork(FACTORY).getPair(WETH, USDC), PAIR, "router would use a different pool");
        (, uint112 r1,) = IV2PairFork(PAIR).getReserves();
        assertGt(uint256(r1), 1000 ether, "pool too thin for this size");

        uint256 ownerBefore = IERC20Fork(WETH).balanceOf(address(this));

        // Front buys USDC: WETH is token1, so direction 0.
        assertTrue(_exec(_steps(0, WETH, FRONT), 0), "front");
        assertTrue(exec.cycleOpen(), "cycle must be open");

        // The victim buys the same direction through the real router.
        vm.startPrank(victim);
        IERC20Fork(WETH).approve(ROUTER, type(uint256).max);
        address[] memory path = new address[](2);
        path[0] = WETH;
        path[1] = USDC;
        IV2RouterFork(ROUTER).swapExactTokensForTokens(
            VICTIM_IN, 0, path, victim, block.timestamp + 1
        );
        vm.stopPrank();

        uint256 held = IERC20Fork(USDC).balanceOf(address(exec));
        assertGt(held, 0, "victim produced nothing for the back leg");

        // Back sells USDC: direction 1, and the cycle must beat the base.
        assertTrue(_exec(_steps(1, USDC, held), 1), "back");

        assertFalse(exec.cycleOpen(), "cycle must close");
        assertGt(IERC20Fork(WETH).balanceOf(address(this)), ownerBefore, "no profit paid");
        assertEq(IERC20Fork(USDC).balanceOf(address(exec)), 0, "USDC dust left");
    }

    function testBackRunWithoutVictimLosesTheTwoFees() public {
        uint256 ownerBefore = IERC20Fork(WETH).balanceOf(address(this));
        assertTrue(_exec(_steps(0, WETH, FRONT), 0), "front");
        uint256 held = IERC20Fork(USDC).balanceOf(address(exec));

        assertTrue(_exec(_steps(1, USDC, held), 0), "close at any result");
        assertFalse(exec.cycleOpen(), "cycle must close");
        assertLt(IERC20Fork(WETH).balanceOf(address(exec)), FLOAT, "float must shrink");
        assertEq(IERC20Fork(WETH).balanceOf(address(this)), ownerBefore, "profit without a victim");
    }
}
