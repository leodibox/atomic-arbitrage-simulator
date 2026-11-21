// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

interface IERC20Like {
    function balanceOf(address) external view returns (uint256);
    function transfer(address, uint256) external returns (bool);
}

interface IV2PairLike {
    function getReserves() external view returns (uint112, uint112, uint32);
    function swap(uint256, uint256, address, bytes calldata) external;
}

/// @title SandwichExecutor
/// @notice Owner-gated V2 sandwich executor. One call is one leg; a sandwich is
///         two calls (front-run, back-run) submitted as a single atomic bundle
///         around the victim transaction.
/// @dev Calldata after the 4-byte selector:
///        [8 bytes blockNumber][N * 105 bytes]
///      105 = direction(1) | pair(20) | tokenIn(20) | amountIn(32) | amountOut(32)
///      direction: 1 => token0 is the input token, 0 => token1 is the input token
///      amountOut == 0 => compute the output from live reserves at 997/1000
///      The encoded blockNumber must equal block.number, so a stale bundle can
///      never trade against reserves it was not priced on.
///      The last leg passes minProfitWeth; anything less reverts and the whole
///      bundle unwinds.
contract SandwichExecutor {
    address public immutable owner;
    address public constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;

    uint256 private constant FEE_NUM = 997;
    uint256 private constant FEE_DEN = 1000;
    uint256 private _lock = 1;
    uint256 private _cycleBase;
    bool private _cycleOpen;

    error NotOwner();
    error Stale();
    error StepFailed();
    error Unprofitable();
    error Reentered();
    error TransferFailed();
    error OpenLegProfit();

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    modifier nonReentrant() {
        if (_lock != 1) revert Reentered();
        _lock = 2;
        _;
        _lock = 1;
    }

    constructor() {
        owner = msg.sender;
    }

    /// @param minProfitWeth net WETH gain required for the WHOLE cycle.
    ///        Opening leg: must be 0, it stores the cycle base.
    ///        Closing leg: 0 closes at any result (used by the unwind),
    ///        a positive value reverts unless the cycle beats base + minimum.
    function execute(uint256 minProfitWeth) external onlyOwner nonReentrant {
        uint256 encodedBlock;
        assembly {
            encodedBlock := shr(192, calldataload(36))
        }
        if (encodedBlock != block.number) revert Stale();

        if (!_cycleOpen) {
            if (minProfitWeth != 0) revert OpenLegProfit();
            _cycleOpen = true;
            _cycleBase = IERC20Like(WETH).balanceOf(address(this));
            _run(msg.data.length);
            return;
        }

        _run(msg.data.length);
        uint256 settled = IERC20Like(WETH).balanceOf(address(this));
        _cycleOpen = false;
        if (minProfitWeth != 0 && settled < _cycleBase + minProfitWeth) revert Unprofitable();
        if (settled > _cycleBase) _safeTransfer(WETH, owner, settled - _cycleBase);
    }

    function _run(uint256 end) private {
        uint256 offset = 44; // selector(4) + minProfit(32) + blockNumber(8)
        while (offset < end) {
            uint256 w0;
            uint256 w1;
            uint256 w2;
            uint256 w3;
            uint256 w4;
            assembly {
                w0 := calldataload(offset)
                w1 := calldataload(add(offset, 1))
                w2 := calldataload(add(offset, 21))
                w3 := calldataload(add(offset, 41))
                w4 := calldataload(add(offset, 73))
            }

            uint8 direction = uint8(w0 >> 248);
            address pair = address(uint160(w1 >> 96));
            address tokenIn = address(uint160(w2 >> 96));
            uint256 amountIn = w3;
            uint256 amountOut = w4;

            if (amountOut == 0) amountOut = _quote(pair, direction, amountIn);
            if (amountIn == 0 || amountOut == 0) revert StepFailed();

            uint256 out0;
            uint256 out1;
            if (direction == 1) {
                out1 = amountOut;
            } else {
                out0 = amountOut;
            }

            _safeTransfer(tokenIn, pair, amountIn);
            IV2PairLike(pair).swap(out0, out1, address(this), "");

            offset += 105;
        }
    }

    function _quote(address pair, uint8 direction, uint256 amountIn)
        private
        view
        returns (uint256 out)
    {
        (uint112 r0, uint112 r1, ) = IV2PairLike(pair).getReserves();
        uint256 rin;
        uint256 rout;
        if (direction == 1) {
            rin = r0;
            rout = r1;
        } else {
            rin = r1;
            rout = r0;
        }
        if (rin == 0 || rout == 0) return 0;
        uint256 feeIn = amountIn * FEE_NUM;
        out = (feeIn * rout) / (rin * FEE_DEN + feeIn);
    }

    function _safeTransfer(address token, address to, uint256 amount) private {
        (bool ok, bytes memory ret) =
            token.call(abi.encodeWithSelector(0xa9059cbb, to, amount));
        if (!ok) revert TransferFailed();
        if (ret.length != 0 && !abi.decode(ret, (bool))) revert TransferFailed();
    }

    function resetCycle() external onlyOwner {
        _cycleOpen = false;
        _cycleBase = 0;
    }

    function cycleOpen() external view returns (bool) {
        return _cycleOpen;
    }

    function sweep(address token, uint256 amount) external onlyOwner {
        _safeTransfer(token, owner, amount);
    }

    function sweepAll(address token) external onlyOwner {
        uint256 bal = IERC20Like(token).balanceOf(address(this));
        if (bal != 0) _safeTransfer(token, owner, bal);
    }

    receive() external payable {}
}
