// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";
import {FlashLoanArb} from "../src/FlashLoanArb.sol";

interface IERC20 {
    function balanceOf(address) external view returns (uint256);
    function transfer(address, uint256) external returns (bool);
    function approve(address, uint256) external returns (bool);
    function mint(address, uint256) external;
}

contract MockERC20 {
    string public name;
    string public symbol;
    uint8 public decimals = 6;
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    constructor(string memory n, string memory s) { name = n; symbol = s; }

    function transfer(address to, uint256 amt) external returns (bool) {
        balanceOf[msg.sender] -= amt;
        balanceOf[to] += amt;
        return true;
    }
    function approve(address spender, uint256 amt) external returns (bool) {
        allowance[msg.sender][spender] = amt;
        return true;
    }
    function transferFrom(address from, address to, uint256 amt) external returns (bool) {
        if (allowance[from][msg.sender] != type(uint256).max) allowance[from][msg.sender] -= amt;
        balanceOf[from] -= amt;
        balanceOf[to] += amt;
        return true;
    }
    function mint(address to, uint256 amt) external { balanceOf[to] += amt; }
}

contract MockPair {
    address public token0;
    address public token1;
    uint112 private r0;
    uint112 private r1;

    constructor(address a, address b, uint112 _r0, uint112 _r1) {
        token0 = a; token1 = b; r0 = _r0; r1 = _r1;
    }
    function getReserves() external view returns (uint112, uint112, uint32) { return (r0, r1, 0); }
    function swap(uint256 amount0Out, uint256 amount1Out, address to, bytes calldata) external {
        if (amount0Out > 0) MockERC20(token0).transfer(to, amount0Out);
        if (amount1Out > 0) MockERC20(token1).transfer(to, amount1Out);
        // update reserves from balances
        r0 = uint112(MockERC20(token0).balanceOf(address(this)));
        r1 = uint112(MockERC20(token1).balanceOf(address(this)));
    }
    function sync(uint112 _r0, uint112 _r1) external { r0 = _r0; r1 = _r1; }
}

contract MockPool {
    uint256 public premiumBps = 5; // 0.05%
    function flashLoanSimple(address receiver, address asset, uint256 amount, bytes calldata params, uint16) external {
        MockERC20(asset).mint(receiver, amount);
        uint256 premium = (amount * premiumBps) / 10000;
        bool ok = FlashLoanArb(payable(receiver)).executeOperation(asset, amount, premium, receiver, params);
        require(ok, "callback failed");
        // pull repayment
        require(MockERC20(asset).transferFrom(receiver, address(this), amount + premium), "repay failed");
    }
}

contract FlashLoanArbProfitTest is Test {
    function test_profitablePathRepaysAndKeepsSurplus() public {
        MockERC20 usdc = new MockERC20("USDC", "USDC");
        MockERC20 weth = new MockERC20("WETH", "WETH");

        // Create a large price gap: pair1 sells WETH cheap (more WETH per USDC),
        // pair2 buys WETH expensive (more USDC per WETH). token0=USDC, token1=WETH.
        MockPair pairCheapWeth = new MockPair(address(usdc), address(weth), 1_000_000e6, 500 ether);
        MockPair pairExpensiveWeth = new MockPair(address(usdc), address(weth), 1_000_000e6, 250 ether);
        usdc.mint(address(pairCheapWeth), 1_000_000e6);
        weth.mint(address(pairCheapWeth), 500 ether);
        usdc.mint(address(pairExpensiveWeth), 1_000_000e6);
        weth.mint(address(pairExpensiveWeth), 250 ether);

        MockPool pool = new MockPool();
        address profitRecipient = address(0xBEEF);
        FlashLoanArb arb = new FlashLoanArb(address(pool), profitRecipient);

        // path: USDC->WETH on pairCheapWeth (zeroForOne=1), WETH->USDC on pairExpensiveWeth (zeroForOne=0)
        bytes memory path = abi.encodePacked(address(pairCheapWeth), uint8(1), address(pairExpensiveWeth), uint8(0));
        uint256 amount = 10_000e6;
        uint256 before = usdc.balanceOf(address(arb));
        arb.executeArbitrage(address(usdc), amount, path, 1);

        uint256 profit = usdc.balanceOf(address(arb)) - before;
        assertGt(profit, 0, "expected positive profit");

        arb.sweep(address(usdc), profit);
        assertEq(usdc.balanceOf(profitRecipient), profit, "recipient received profit");
    }
}
