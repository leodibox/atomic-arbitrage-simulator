// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

/// @notice Mainnet-oriented candidate for review; NOT audited or deployed.
/// Supports only previously approved V2-compatible pairs using 30 bps fee.
/// Min-profit is denominated in the borrowed asset. Off-chain gas, ETH funding,
/// EIP-1559 fee caps, nonce fencing, and exact eth_call remain mandatory.
interface IGuardedToken {
    function balanceOf(address) external view returns (uint256);
    function transfer(address, uint256) external returns (bool);
    function approve(address, uint256) external returns (bool);
}
interface IGuardedAavePool {
    function flashLoanSimple(
        address receiverAddress, address asset, uint256 amount,
        bytes calldata params, uint16 referralCode
    ) external;
}
interface IGuardedV2Pair {
    function swap(uint256, uint256, address, bytes calldata) external;
    function getReserves() external view returns (uint112, uint112, uint32);
    function token0() external view returns (address);
    function token1() external view returns (address);
}

contract FlashLoanArbGuarded {
    address public immutable owner;
    address public immutable profitRecipient;
    IGuardedAavePool public immutable pool;
    bool public paused = true;
    // Opt-in only: funds stay in the contract unless the owner explicitly
    // enables automatic payout to the immutable recipient after Aave repays.
    bool public autoPayout;

    // All are opt-in. Zero maxLoanRaw disables a flash-loan asset.
    mapping(address => uint256) public maxLoanRaw;
    mapping(address => bool) public approvedPair;
    bool private inFlight;

    error NotOwner();
    error NotPool();
    error Paused();
    error Reentrant();
    error BadPath();
    error BadAsset();
    error UnapprovedPair();
    error LoanCapExceeded();
    error Unprofitable();
    error TransferFailed();
    error ZeroAddress();

    event PausedSet(bool paused);
    event AutoPayoutSet(bool enabled);
    event ProfitPaid(address indexed asset, address indexed recipient, uint256 amount);
    event LoanCapSet(address indexed asset, uint256 capRaw);
    event PairApprovalSet(address indexed pair, bool allowed);
    event FlashLoanExecuted(address indexed asset, uint256 borrowed, uint256 premium, uint256 profit);
    event ProfitSwept(address indexed token, uint256 amount);

    constructor(address aavePool, address recipient) {
        if (aavePool == address(0) || recipient == address(0)) revert ZeroAddress();
        if (aavePool.code.length == 0) revert BadAsset();
        owner = msg.sender;
        profitRecipient = recipient;
        pool = IGuardedAavePool(aavePool);
    }

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    function setPaused(bool value) external onlyOwner {
        paused = value;
        emit PausedSet(value);
    }

    /// @notice Off by default; only the immutable profitRecipient can receive
    /// realized gains automatically, and only after Aave takes repayment.
    function setAutoPayout(bool enabled) external onlyOwner {
        if (inFlight) revert Reentrant();
        autoPayout = enabled;
        emit AutoPayoutSet(enabled);
    }

    function setLoanCap(address asset, uint256 capRaw) external onlyOwner {
        if (asset == address(0) || asset.code.length == 0) revert BadAsset();
        maxLoanRaw[asset] = capRaw;
        emit LoanCapSet(asset, capRaw);
    }

    function setApprovedPair(address pair, bool allowed) external onlyOwner {
        if (pair == address(0) || (allowed && pair.code.length == 0)) revert BadPath();
        approvedPair[pair] = allowed;
        emit PairApprovalSet(pair, allowed);
    }

    /// @notice Only a reviewed V2 path with 2-3 distinct pairs.
    /// Path = repeated [pair address (20 bytes), zeroForOne (0 or 1 byte)].
    function executeArbitrage(
        address asset,
        uint256 amount,
        bytes calldata path,
        uint256 minProfit
    ) external onlyOwner {
        if (paused) revert Paused();
        if (inFlight) revert Reentrant();
        if (asset == address(0) || asset.code.length == 0) revert BadAsset();
        if (maxLoanRaw[asset] == 0 || amount == 0 || amount > maxLoanRaw[asset])
            revert LoanCapExceeded();
        if (minProfit == 0 || (path.length != 42 && path.length != 63))
            revert BadPath();

        // FlashLoanSimple repays by pulling amount+premium AFTER executeOperation
        // returns. Only count and transfer the new balance AFTER the pool call
        // returns successfully, never before its repayment.
        uint256 originalBalance = IGuardedToken(asset).balanceOf(address(this));
        inFlight = true;
        pool.flashLoanSimple(address(this), asset, amount, abi.encode(path, minProfit), 0);
        uint256 settledBalance = IGuardedToken(asset).balanceOf(address(this));
        if (settledBalance <= originalBalance) revert Unprofitable();
        uint256 realizedProfit = settledBalance - originalBalance;
        if (realizedProfit < minProfit) revert Unprofitable();
        if (autoPayout) {
            _safeTransfer(asset, profitRecipient, realizedProfit);
            emit ProfitPaid(asset, profitRecipient, realizedProfit);
        }
        inFlight = false;
    }

    /// @notice Only Aave may invoke during an active owner-initiated loan.
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external returns (bool) {
        if (msg.sender != address(pool)) revert NotPool();
        if (!inFlight || initiator != address(this)) revert Reentrant();
        if (paused) revert Paused();
        if (maxLoanRaw[asset] == 0 || amount == 0 || amount > maxLoanRaw[asset])
            revert LoanCapExceeded();

        (bytes memory path, uint256 minProfit) = abi.decode(params, (bytes, uint256));
        if ((path.length != 42 && path.length != 63) || minProfit == 0) revert BadPath();

        uint256 startBalance = IGuardedToken(asset).balanceOf(address(this));
        if (startBalance < amount) revert Unprofitable();
        uint256 priorBalance = startBalance - amount;

        uint256 currentAmount = amount;
        address currentToken = asset;
        address[] memory seen = new address[](path.length / 21);

        for (uint256 i = 0; i < seen.length; ++i) {
            uint256 offset = i * 21;
            address pairAddress;
            uint8 direction;
            assembly {
                let base := add(path, 32)
                pairAddress := shr(96, mload(add(base, offset)))
                direction := byte(0, mload(add(base, add(offset, 20))))
            }

            if (pairAddress == address(0) || direction > 1) revert BadPath();
            if (!approvedPair[pairAddress]) revert UnapprovedPair();
            for (uint256 j = 0; j < i; ++j) {
                if (seen[j] == pairAddress) revert BadPath();
            }
            seen[i] = pairAddress;

            (currentToken, currentAmount) =
                _executeV2Hop(pairAddress, direction == 1, currentToken, currentAmount);
        }
        if (currentToken != asset) revert BadPath();

        uint256 owed = amount + premium;
        uint256 finalBalance = IGuardedToken(asset).balanceOf(address(this));
        // On-chain asset surplus; *does not* pay or measure the owner's ETH gas.
        if (finalBalance < priorBalance + owed + minProfit) revert Unprofitable();
        _safeApprove(asset, address(pool), 0);
        _safeApprove(asset, address(pool), owed);
        emit FlashLoanExecuted(asset, amount, premium, finalBalance - priorBalance - owed);
        return true;
    }

    function _executeV2Hop(
        address pairAddress,
        bool zeroForOne,
        address currentToken,
        uint256 inputAmount
    ) private returns (address outputToken, uint256 received) {
        IGuardedV2Pair pair = IGuardedV2Pair(pairAddress);
        address token0 = pair.token0();
        address token1 = pair.token1();
        if (zeroForOne ? currentToken != token0 : currentToken != token1)
            revert BadPath();
        outputToken = zeroForOne ? token1 : token0;
        if (token0 == token1 || outputToken == address(0)) revert BadPath();

        uint256 output = _quoteV2(pair, zeroForOne, inputAmount);
        uint256 beforeOut = IGuardedToken(outputToken).balanceOf(address(this));
        _safeTransfer(currentToken, pairAddress, inputAmount);
        pair.swap(zeroForOne ? 0 : output, zeroForOne ? output : 0, address(this), "");
        uint256 afterOut = IGuardedToken(outputToken).balanceOf(address(this));
        if (afterOut < beforeOut + output) revert TransferFailed();
        received = afterOut - beforeOut;
    }

    function _quoteV2(IGuardedV2Pair pair, bool zeroForOne, uint256 amount)
        private view returns (uint256 output)
    {
        (uint112 r0, uint112 r1,) = pair.getReserves();
        (uint256 rIn, uint256 rOut) =
            zeroForOne ? (uint256(r0), uint256(r1)) : (uint256(r1), uint256(r0));
        if (rIn == 0 || rOut == 0 || amount == 0) revert BadPath();
        // Applies canonical V2 30 bps fee; approved pair config MUST exclude
        // fee-customized forks or any nonstandard/malicious AMM implementation.
        uint256 feeAmount = amount * 997;
        output = (feeAmount * rOut) / (rIn * 1000 + feeAmount);
        if (output == 0 || output >= rOut) revert Unprofitable();
    }

    function sweep(address token, uint256 amount) external onlyOwner {
        if (inFlight) revert Reentrant();
        if (token == address(0)) revert BadAsset();
        _safeTransfer(token, profitRecipient, amount);
        emit ProfitSwept(token, amount);
    }

    function _safeTransfer(address token, address to, uint256 amount) internal {
        if (token.code.length == 0) revert BadAsset();
        (bool success, bytes memory returndata) = token.call(
            abi.encodeWithSelector(IGuardedToken.transfer.selector, to, amount)
        );
        if (!success || (returndata.length != 0 &&
            (returndata.length != 32 || !abi.decode(returndata, (bool)))))
            revert TransferFailed();
    }

    function _safeApprove(address token, address spender, uint256 amount) internal {
        if (token.code.length == 0) revert BadAsset();
        (bool success, bytes memory returndata) = token.call(
            abi.encodeWithSelector(IGuardedToken.approve.selector, spender, amount)
        );
        if (!success || (returndata.length != 0 &&
            (returndata.length != 32 || !abi.decode(returndata, (bool)))))
            revert TransferFailed();
    }
}
