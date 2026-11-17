// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

interface TokenR {
    function balanceOf(address) external view returns (uint256);
    function allowance(address, address) external view returns (uint256);
    function approve(address, uint256) external returns (bool);
    function transfer(address, uint256) external returns (bool);
    function deposit() external payable;
}
interface MorphoR { function flashLoan(address, uint256, bytes calldata) external; }
interface FactoryR { function getPool(address, address, uint24) external view returns (address); }
interface PoolR {
    function factory() external view returns (address);
    function token0() external view returns (address);
    function token1() external view returns (address);
    function fee() external view returns (uint24);
    function liquidity() external view returns (uint128);
    function tickSpacing() external view returns (int24);
    function slot0() external view returns (uint160, int24, uint16, uint16, uint16, uint8, bool);
    function feeGrowthGlobal0X128() external view returns (uint256);
    function feeGrowthGlobal1X128() external view returns (uint256);
    function swap(address, bool, int256, uint160, bytes calldata) external returns (int256, int256);
}
interface PairR {
    function token0() external view returns (address);
    function token1() external view returns (address);
    function factory() external view returns (address);
    function getReserves() external view returns (uint112, uint112, uint32);
    function swap(uint256, uint256, address, bytes calldata) external;
}
interface V2FactoryR { function getPair(address,address) external view returns (address); }

/// FORK-ONLY RESEARCH FIXTURE. Not a production executor or deployment proposal.
contract ResearchReceiver {
    address public constant MORPHO = 0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb;
    address public constant FACTORY = 0x1F98431c8aD98523631AE4a59f267346ea31F984;
    uint160 internal constant MIN = 4295128740;
    uint160 internal constant MAX = 1461446703485210103287273052203988822378723970341;
    address public immutable owner = msg.sender;
    address private loanToken;
    uint256 private loanAmount;
    uint8 private mode;
    address private firstPool;
    address private secondPool;
    bool private secondIsV2;
    address private activePool;
    uint256 private budget;
    bool private shortPay;
    uint256 public callbackBalance;
    uint256 public paid;
    int256 public lastAmount0;
    int256 public lastAmount1;
    int256 public routeMargin;

    error Unauthorized();
    error Inactive();
    error BadPool();
    error BadDelta();
    error PartialFill();
    error Slippage();
    error IntentionalCallbackFailure();
    error Unprofitable(int256 margin);

    modifier onlyOwner() { if(msg.sender != owner) revert Unauthorized(); _; }

    function borrow(address token, uint256 amount, uint8 failureMode) external onlyOwner {
        if (loanToken != address(0)) revert Inactive();
        loanToken = token; loanAmount = amount; mode = failureMode;
        MorphoR(MORPHO).flashLoan(token, amount, "");
        require(TokenR(token).approve(MORPHO, 0));
        loanToken = address(0); loanAmount = 0;
    }

    function roundTrip(address token, uint256 amount, address first, address second, bool v2) external onlyOwner {
        if (loanToken != address(0)) revert Inactive();
        uint256 beforeBalance = TokenR(token).balanceOf(address(this));
        loanToken = token; loanAmount = amount; mode = 3;
        firstPool = first; secondPool = second; secondIsV2 = v2;
        MorphoR(MORPHO).flashLoan(token, amount, "");
        require(TokenR(token).approve(MORPHO, 0));
        routeMargin = int256(TokenR(token).balanceOf(address(this))) - int256(beforeBalance);
        loanToken = address(0); loanAmount = 0;
    }

    function onMorphoFlashLoan(uint256 assets, bytes calldata) external {
        if (msg.sender != MORPHO) revert Unauthorized();
        if (loanToken == address(0) || assets != loanAmount) revert Inactive();
        callbackBalance = TokenR(loanToken).balanceOf(address(this));
        require(callbackBalance >= assets, "principal missing");
        if (mode == 1) revert IntentionalCallbackFailure();
        if (mode == 3) {
            address intermediate = loanToken == PoolR(firstPool).token0()
                ? PoolR(firstPool).token1() : PoolR(firstPool).token0();
            uint256 output = _swap(firstPool, loanToken == PoolR(firstPool).token0(), assets, 0, 0, false);
            if (secondIsV2) _v2(secondPool, intermediate, output);
            else _swap(secondPool, intermediate == PoolR(secondPool).token0(), output, 0, 0, false);
            // Do not subsidize a loss with any pre-existing float.
            int256 margin = int256(TokenR(loanToken).balanceOf(address(this))) - int256(callbackBalance);
            if (margin < 0) revert Unprofitable(margin);
        }
        require(TokenR(loanToken).approve(MORPHO, mode == 2 ? assets - 1 : assets));
    }

    function swapExactIn(address pool, bool zeroForOne, uint256 amount, uint256 minOut, uint160 limit, bool underpay)
        external onlyOwner returns (uint256)
    { return _swap(pool, zeroForOne, amount, minOut, limit, underpay); }

    function _swap(address pool, bool zeroForOne, uint256 amount, uint256 minOut, uint160 limit, bool underpay)
        private returns (uint256 out)
    {
        if (activePool != address(0)) revert Inactive();
        PoolR p = PoolR(pool);
        if (p.factory() != FACTORY || FactoryR(FACTORY).getPool(p.token0(), p.token1(), p.fee()) != pool)
            revert BadPool();
        require(amount > 0 && amount <= uint256(type(int256).max));
        activePool = pool; budget = amount; paid = 0; shortPay = underpay;
        (int256 a0, int256 a1) = p.swap(address(this), zeroForOne, int256(amount),
            limit == 0 ? (zeroForOne ? MIN : MAX) : limit, "");
        if (paid != amount) revert PartialFill();
        out = uint256(-(zeroForOne ? a1 : a0));
        if (out < minOut) revert Slippage();
        lastAmount0 = a0; lastAmount1 = a1;
        activePool = address(0); budget = 0;
    }

    function uniswapV3SwapCallback(int256 a0, int256 a1, bytes calldata) external {
        if (msg.sender != activePool || activePool == address(0)) revert Unauthorized();
        PoolR p = PoolR(msg.sender);
        if (FactoryR(FACTORY).getPool(p.token0(), p.token1(), p.fee()) != msg.sender) revert BadPool();
        if (!((a0 > 0 && a1 <= 0) || (a1 > 0 && a0 <= 0))) revert BadDelta();
        uint256 owed = uint256(a0 > 0 ? a0 : a1);
        if (owed > budget || paid != 0) revert BadDelta();
        paid = owed;
        require(TokenR(a0 > 0 ? p.token0() : p.token1()).transfer(msg.sender, shortPay ? owed - 1 : owed));
    }

    function _v2(address pair, address input, uint256 amount) private {
        PairR p = PairR(pair);
        address fac = p.factory();
        if (fac != 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f &&
            fac != 0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac) revert BadPool();
        if (V2FactoryR(fac).getPair(p.token0(), p.token1()) != pair) revert BadPool();
        bool zeroForOne = input == p.token0();
        require(zeroForOne || input == p.token1());
        (uint112 r0, uint112 r1,) = p.getReserves();
        (uint256 ri,uint256 ro) = zeroForOne ? (uint256(r0),uint256(r1)) : (uint256(r1),uint256(r0));
        uint256 out = amount * 997 * ro / (ri * 1000 + amount * 997);
        require(TokenR(input).transfer(pair, amount));
        p.swap(zeroForOne ? 0 : out, zeroForOne ? out : 0, address(this), "");
    }
}
