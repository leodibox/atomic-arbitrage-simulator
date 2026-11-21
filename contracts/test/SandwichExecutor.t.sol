// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {SandwichExecutor} from "../src/SandwichExecutor.sol";

contract MockToken {
    mapping(address => uint256) public balanceOf;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        require(balanceOf[msg.sender] >= amount, "bal");
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}

contract MockPair {
    MockToken public token0;
    MockToken public token1;

    constructor(MockToken t0, MockToken t1) {
        token0 = t0;
        token1 = t1;
    }

    function getReserves() external view returns (uint112, uint112, uint32) {
        return (
            uint112(token0.balanceOf(address(this))),
            uint112(token1.balanceOf(address(this))),
            0
        );
    }

    function swap(uint256 a0, uint256 a1, address to, bytes calldata) external {
        if (a0 != 0) token0.transfer(to, a0);
        if (a1 != 0) token1.transfer(to, a1);
    }
}

contract SandwichExecutorTest is Test {
    SandwichExecutor exec;
    MockToken weth;
    MockToken token;
    MockPair pair;

    function setUp() public {
        exec = new SandwichExecutor();
        MockToken wethImpl = new MockToken();
        vm.etch(0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2, address(wethImpl).code);
        weth = MockToken(0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2);
        token = new MockToken();
        pair = new MockPair(weth, token);

        weth.mint(address(pair), 1000 ether);
        token.mint(address(pair), 1_000_000 ether);
        weth.mint(address(exec), 10 ether);
    }

    function _steps(uint8 dir, address tokenIn, uint256 amountIn)
        internal
        view
        returns (bytes memory)
    {
        return abi.encodePacked(
            uint64(block.number),
            dir,
            address(pair),
            tokenIn,
            amountIn,
            uint256(0)
        );
    }

    function _exec(bytes memory steps, uint256 minProfit) internal returns (bool ok) {
        bytes memory cd = abi.encodePacked(
            abi.encodeWithSelector(SandwichExecutor.execute.selector, minProfit),
            steps
        );
        (ok, ) = address(exec).call(cd);
    }

    function testRejectsNonOwner() public {
        vm.prank(address(0xBEEF));
        vm.expectRevert(SandwichExecutor.NotOwner.selector);
        _exec(_steps(1, address(weth), 1 ether), 0);
    }

    function testRejectsStaleBlock() public {
        bytes memory steps = abi.encodePacked(
            uint64(block.number + 5),
            uint8(1),
            address(pair),
            address(weth),
            uint256(1 ether),
            uint256(0)
        );
        vm.expectRevert(SandwichExecutor.Stale.selector);
        _exec(steps, 0);
    }

    function testFrontThenBackExtractsProfit() public {
        assertTrue(_exec(_steps(1, address(weth), 1 ether), 0));
        weth.mint(address(pair), 200 ether);

        // victim buys: WETH lands in the pool and lifts the token price

        uint256 held = token.balanceOf(address(exec));
        assertGt(held, 0);
        assertTrue(_exec(_steps(0, address(token), held), 1));

        assertGt(weth.balanceOf(address(this)), 0);
        assertEq(token.balanceOf(address(exec)), 0);
    }

    function testBackRunRevertsWhenMinProfitUnmet() public {
        _exec(_steps(1, address(weth), 1 ether), 0);
        uint256 held = token.balanceOf(address(exec));
        vm.expectRevert(SandwichExecutor.Unprofitable.selector);
        _exec(_steps(0, address(token), held), 1 ether);
    }

    function testSweepAllReturnsFloat() public {
        exec.sweepAll(address(weth));
        assertEq(weth.balanceOf(address(this)), 10 ether);
    }
}