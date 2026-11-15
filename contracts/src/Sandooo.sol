// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

/// Instruction stream: [8 bytes blockNumber][N * 105 bytes]
/// 105 = zeroForOne(1) | pair(20) | tokenIn(20) | amountIn(32) | amountOut(32)
contract Sandooo {
    address public immutable owner;
    error NotOwner();
    error Stale();
    error SwapFailed();

    uint256 private constant TRANSFER = 0xa9059cbb << 224;
    uint256 private constant SWAP = 0x022c0d9f << 224;

    constructor() { owner = msg.sender; }

    fallback() external {
        if (msg.sender != owner) revert NotOwner();
        assembly {
            let bn := shr(192, calldataload(0))
            if iszero(eq(bn, number())) { mstore(0, 0) revert(0, 0) }

            let offset := 8
            let end := calldatasize()
            for {} lt(offset, end) {} {
                let dir := byte(0, calldataload(offset))
                let pair := shr(96, calldataload(add(offset, 1)))
                let tokenIn := shr(96, calldataload(add(offset, 21)))
                let amountIn := calldataload(add(offset, 41))
                let amountOut := calldataload(add(offset, 73))

                mstore(0, TRANSFER)
                mstore(4, pair)
                mstore(36, amountIn)
                if iszero(call(gas(), tokenIn, 0, 0, 68, 0, 0)) { revert(0, 0) }

                let a0 := 0
                let a1 := 0
                switch dir
                case 0 { a0 := amountOut }
                default { a1 := amountOut }

                mstore(0, SWAP)
                mstore(4, a0)
                mstore(36, a1)
                mstore(68, address())
                mstore(100, 128)
                mstore(132, 0)
                if iszero(call(gas(), pair, 0, 0, 164, 0, 0)) { revert(0, 0) }

                offset := add(offset, 105)
            }
        }
    }

    function sweep(address token, uint256 amount) external {
        if (msg.sender != owner) revert NotOwner();
        (bool ok,) = token.call(abi.encodeWithSelector(0xa9059cbb, owner, amount));
        require(ok, "sweep");
    }

    receive() external payable {}
}
