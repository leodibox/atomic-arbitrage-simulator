// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";

interface ITriangleFactory {
    function getPair(address, address) external view returns (address);
}
interface ITrianglePair {
    function factory() external view returns (address);
    function token0() external view returns (address);
    function token1() external view returns (address);
    function getReserves() external view returns (uint112, uint112, uint32);
}
interface ITriangleAave {
    function FLASHLOAN_PREMIUM_TOTAL() external view returns (uint128);
}
interface ITriangleEthUsd {
    function decimals() external view returns (uint8);
    function latestRoundData() external view returns (
        uint80, int256, uint256, uint256, uint80
    );
}

/// Audit of real V2 3-hop WETH->tokenA->tokenB->WETH cycles.
/// Adds non-WETH cross-token pairs omitted by WETH-star discovery.
/// No deals, mocked reserves, senders, contract activation, or broadcast.
contract TriangularV2RealPairsForkTest is Test {
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    address constant DAI = 0x6B175474E89094C44Da98b954EedeAC495271d0F;
    address constant WBTC = 0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599;
    address constant LINK = 0x514910771AF9Ca656af840dff83E8264EcF986CA;
    address constant UNI = 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f;
    address constant SUSHI = 0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac;
    address constant AAVE = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2;
    address constant ORACLE = 0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419;
    uint256 constant MIN_WETH = 25 ether;
    uint256 constant GAS_LIMIT = 1_000_000;
    uint256 constant GAS_TIP = 2 gwei;
    uint256 constant HAIRCUT_BPS = 10;
    uint256 constant DOLLAR_E8 = 100_000_000;

    struct Pair {
        address addr;
        address token0;
        address token1;
        uint256 reserve0;
        uint256 reserve1;
        bool nonzero;
        bool enoughWeth;
    }
    struct Cost {
        uint256 premiumBps;
        uint256 gasWei;
        uint256 gasUsdE8;
    }
    struct Result {
        uint256 pairsFound;
        uint256 eligibleTriangles;
        uint256 probes;
        uint256 grossPositive;
        uint256 netPositive;
        uint256 within5;
        uint256 within10;
        uint256 within15;
        uint256 within20;
    }

    function _fork() private {
        string memory rpc = vm.envOr("HTTPS_URL", string(""));
        if (bytes(rpc).length == 0) vm.skip(true);
        uint256 n = vm.envOr("MAINNET_FORK_BLOCK", uint256(0));
        if (n == 0) vm.createSelectFork(rpc);
        else vm.createSelectFork(rpc, n);
        assertEq(block.chainid, 1);
    }

    function _pair(address factory, address tokenA, address tokenB)
        private view returns (Pair memory p)
    {
        address addr = ITriangleFactory(factory).getPair(tokenA, tokenB);
        if (addr == address(0)) return p;
        assertGt(addr.code.length, 0, "factory pair missing bytecode");
        ITrianglePair v2 = ITrianglePair(addr);
        assertEq(v2.factory(), factory, "factory mismatch");
        p.token0 = v2.token0();
        p.token1 = v2.token1();
        assertTrue(
            (p.token0 == tokenA && p.token1 == tokenB)
                || (p.token0 == tokenB && p.token1 == tokenA),
            "onchain token mismatch"
        );
        (uint112 r0, uint112 r1,) = v2.getReserves();
        p.addr = addr;
        p.reserve0 = uint256(r0);
        p.reserve1 = uint256(r1);
        p.nonzero = r0 > 0 && r1 > 0;
        uint256 wethReserve = p.token0 == WETH ? uint256(r0) :
            p.token1 == WETH ? uint256(r1) : 0;
        p.enoughWeth = wethReserve >= MIN_WETH;
    }

    function _quote(Pair memory p, address assetIn, uint256 amount)
        private pure returns (uint256)
    {
        if (!p.nonzero || amount == 0) return 0;
        uint256 rIn;
        uint256 rOut;
        if (assetIn == p.token0) {
            rIn = p.reserve0;
            rOut = p.reserve1;
        } else if (assetIn == p.token1) {
            rIn = p.reserve1;
            rOut = p.reserve0;
        } else return 0;
        return amount * 997 * rOut / (rIn * 1000 + amount * 997);
    }

    function _maxGasUsd() private view returns (uint256 gasWei, uint256 usdE8) {
        ITriangleEthUsd feed = ITriangleEthUsd(ORACLE);
        require(feed.decimals() == 8, "unexpected oracle decimals");
        (uint80 roundId, int256 answer,, uint256 updated, uint80 answered) =
            feed.latestRoundData();
        require(roundId != 0 && answered >= roundId && answer > 0, "bad oracle round");
        require(updated > 0 && updated <= block.timestamp
            && block.timestamp - updated <= 3600, "oracle stale");
        gasWei = GAS_LIMIT * (2 * block.basefee + GAS_TIP);
        uint256 n = gasWei * uint256(answer) * 110;
        uint256 d = 1 ether * 100;
        usdE8 = (n + d - 1) / d;
    }

    function _count(
        Pair memory a, Pair memory b, Pair memory c,
        address firstToken, address secondToken,
        Cost memory cost, Result memory total
    ) private pure returns (Result memory) {
        // First/last WETH edges must each have >=25 WETH.
        // Middle non-WETH cross-token pair must be nonzero, with factory
        // and token identity proven. No claim that this proves safe depth.
        if (!a.nonzero || !a.enoughWeth || !b.nonzero
            || !c.nonzero || !c.enoughWeth) return total;
        if (a.addr == b.addr || a.addr == c.addr || b.addr == c.addr) return total;
        total.eligibleTriangles++;
        for (uint256 i; i < 10; ++i) {
            uint256 pct = i == 0 ? 100 : i == 1 ? 50 :
                i == 2 ? 40 : i == 3 ? 30 : i == 4 ? 25 :
                i == 5 ? 20 : i == 6 ? 10 : i == 7 ? 5 :
                i == 8 ? 2 : 1;
            // Exactly same 0.25 WETH max and 10 percentages as NetScan.
            uint256 amount = (0.25 ether * pct) / 100;
            uint256 afterA = _quote(a, WETH, amount);
            uint256 afterB = _quote(b, firstToken, afterA);
            uint256 afterC = _quote(c, secondToken, afterB);
            total.probes++;
            if (afterC <= amount) continue;
            total.grossPositive++;
            uint256 premiumWei = (amount * cost.premiumBps + 9999) / 10000;
            uint256 buffered = afterC * (10000 - HAIRCUT_BPS) / 10000;
            if (buffered <= amount + premiumWei + cost.gasWei) continue;
            total.netPositive++;
            if (cost.gasUsdE8 <= 5 * DOLLAR_E8) total.within5++;
            if (cost.gasUsdE8 <= 10 * DOLLAR_E8) total.within10++;
            if (cost.gasUsdE8 <= 15 * DOLLAR_E8) total.within15++;
            if (cost.gasUsdE8 <= 20 * DOLLAR_E8) total.within20++;
        }
        return total;
    }

    function testFork_realCrossTokenTrianglesAndFiveDollarCap() public {
        _fork();
        (uint256 gasWei, uint256 usdGas) = _maxGasUsd();
        uint256 premium = ITriangleAave(AAVE).FLASHLOAN_PREMIUM_TOTAL();
        assertTrue(premium >= 1 && premium <= 1000, "invalid Aave fee");
        address[4] memory markets = [USDC, DAI, WBTC, LINK];
        address[2] memory factories = [UNI, SUSHI];
        Result memory total;
        Cost memory cost = Cost(premium, gasWei, usdGas);
        // Cross-token pools plus WETH spokes. Six token combinations;
        // 2x2x2 factory combinations, both cyclic orientations each.
        for (uint256 i; i < markets.length; ++i) {
            for (uint256 j = i + 1; j < markets.length; ++j) {
                Pair[2] memory wA;
                Pair[2] memory wB;
                Pair[2] memory ab;
                for (uint256 f; f < 2; ++f) {
                    wA[f] = _pair(factories[f], WETH, markets[i]);
                    wB[f] = _pair(factories[f], WETH, markets[j]);
                    ab[f] = _pair(factories[f], markets[i], markets[j]);
                    if (ab[f].addr != address(0)) total.pairsFound++;
                    emit log_named_address("cross_token_A", markets[i]);
                    emit log_named_address("cross_token_B", markets[j]);
                    emit log_named_address("cross_pair_or_zero", ab[f].addr);
                    emit log_named_uint("cross_pair_nonzero", ab[f].nonzero ? 1 : 0);
                }
                for (uint256 a; a < 2; ++a) {
                    for (uint256 b; b < 2; ++b) {
                        for (uint256 c; c < 2; ++c) {
                            total = _count(wA[a], ab[b], wB[c],
                                markets[i], markets[j], cost, total);
                            total = _count(wB[a], ab[b], wA[c],
                                markets[j], markets[i], cost, total);
                        }
                    }
                }
            }
        }
        emit log_named_uint("fork_block", block.number);
        emit log_named_uint("cross_token_pairs_found_across_factories", total.pairsFound);
        emit log_named_uint("eligible_three_hop_route_variants", total.eligibleTriangles);
        emit log_named_uint("probes_evaluated", total.probes);
        emit log_named_uint("gross_positive_probes", total.grossPositive);
        emit log_named_uint("net_positive_probes", total.netPositive);
        emit log_named_uint("worst_case_gas_wei", gasWei);
        emit log_named_uint("worst_case_gas_usd_e8", usdGas);
        emit log_named_uint("within_real_sender_five_usd_limit", usdGas <= 500_000_000 ? 1 : 0);
        emit log_named_uint("net_probes_within_5_usd", total.within5);
        emit log_named_uint("net_probes_within_10_usd", total.within10);
        emit log_named_uint("net_probes_within_15_usd", total.within15);
        emit log_named_uint("net_probes_within_20_usd", total.within20);
        emit log_string("READ ONLY: NOT executable or authorized routes; no exact contract eth_call");
        assertLe(total.within5, total.within10);
        assertLe(total.within10, total.within15);
        assertLe(total.within15, total.within20);
        assertLe(total.within20, total.netPositive);
    }
}
