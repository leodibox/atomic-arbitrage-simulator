// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

interface IERC20 {
    function balanceOf(address) external view returns (uint256);
    function transfer(address, uint256) external returns (bool);
    function approve(address, uint256) external returns (bool);
}

interface IPool {
    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 referralCode
    ) external;

    function FLASHLOAN_PREMIUM_TOTAL() external view returns (uint128);
}

interface IPair {
    function swap(uint256, uint256, address, bytes calldata) external;
    function getReserves() external view returns (uint112, uint112, uint32);
    function token0() external view returns (address);
    function token1() external view returns (address);
}

/// Aave V3 flash-loan receiver. Borrows `amount` of `asset`, runs an atomic
/// multi-hop arbitrage, repays `amount + premium`, keeps the surplus.
/// Simulation-only: owner-gated, no mainnet submission from the test harness.
contract FlashLoanArb {
    address public immutable owner;
    address public immutable profitRecipient;
    IPool public immutable pool;

    error NotOwner();
    error NotPool();
    error Unprofitable();
    error BadPath();
    error InsufficientRepayment();
    error ZeroAddress();

    event FlashLoanExecuted(address indexed asset, uint256 amount, uint256 premium, int256 profit);

    constructor(address _pool, address _profitRecipient) {
        if (_profitRecipient == address(0)) revert ZeroAddress();
        owner = msg.sender;
        profitRecipient = _profitRecipient;
        pool = IPool(_pool);
    }

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    /// Initiate a flash loan. `path` is [pair(20)|zeroForOne(1)] repeated.
    /// `minProfit` is the minimum net profit in `asset` units required to proceed.
    function executeArbitrage(address asset, uint256 amount, bytes calldata path, uint256 minProfit)
        external
        onlyOwner
    {
        if (path.length == 0 || path.length % 21 != 0) revert BadPath();
        bytes memory params = abi.encode(path, minProfit);
        pool.flashLoanSimple(address(this), asset, amount, params, 0);
    }

    /// Aave V3 callback. Must repay amount + premium before returning true.
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external returns (bool) {
        if (msg.sender != address(pool)) revert NotPool();
        if (initiator != address(this)) revert NotOwner();

        (bytes memory path, uint256 minProfit) = abi.decode(params, (bytes, uint256));

        uint256 startBal = IERC20(asset).balanceOf(address(this));
        uint256 cur = amount;
        address curToken = asset;

        for (uint256 i = 0; i < path.length; i += 21) {
            address pair;
            bool zeroForOne;
            assembly {
                let base := add(path, 32)
                pair := shr(96, mload(add(base, i)))
                zeroForOne := eq(byte(0, mload(add(base, add(i, 20)))), 1)
            }

            (uint112 r0, uint112 r1,) = IPair(pair).getReserves();
            (uint256 rIn, uint256 rOut) = zeroForOne ? (r0, r1) : (r1, r0);
            uint256 out = (cur * 997 * rOut) / (rIn * 1000 + cur * 997);

            IERC20(curToken).transfer(pair, cur);
            IPair(pair).swap(zeroForOne ? 0 : out, zeroForOne ? out : 0, address(this), "");

            cur = out;
            curToken = zeroForOne ? IPair(pair).token1() : IPair(pair).token0();
        }

        if (curToken != asset) revert BadPath();

        {
            uint256 endBal = IERC20(asset).balanceOf(address(this));
            uint256 owed = amount + premium;
            uint256 preExisting = startBal >= amount ? startBal - amount : 0;
            if (endBal < preExisting + owed + minProfit) revert Unprofitable();
            IERC20(asset).approve(address(pool), owed);
            emit FlashLoanExecuted(asset, amount, premium, int256(endBal - preExisting - owed));
        }
        return true;
    }

    /// Sweep profits to the authorized recipient. Only owner can call.
    function sweep(address token, uint256 amount) external onlyOwner {
        IERC20(token).transfer(profitRecipient, amount);
    }

    receive() external payable {}
}
