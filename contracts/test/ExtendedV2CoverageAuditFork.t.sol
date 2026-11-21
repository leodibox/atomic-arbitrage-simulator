// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import {Test} from "forge-std/Test.sol";

interface IAuditFactory {
    function getPair(address, address) external view returns (address);
}
interface IAuditPair {
    function factory() external view returns (address);
    function token0() external view returns (address);
    function token1() external view returns (address);
    function getReserves() external view returns (uint112, uint112, uint32);
}
interface IAuditAave {
    function FLASHLOAN_PREMIUM_TOTAL() external view returns (uint128);
}
interface IAuditEthUsd {
    function decimals() external view returns (uint8);
    function latestRoundData() external view returns (
        uint80, int256, uint256, uint256, uint80
    );
}

/// Reports real reserves for NEW candidate WETH V2 markets only.
/// Does NOT change the production scanner allowlist, any contract, or
/// authorization. Quoted profits are counterfactual same-block estimates.
/// No signer, broadcast, deal, mint, or manipulated reserves in this test.
contract ExtendedV2CoverageAuditForkTest is Test {
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant AAVE_POOL = 0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2;
    address constant UNI_FACTORY = 0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f;
    address constant SUSHI_FACTORY = 0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac;
    address constant CHAINLINK_ETH_USD = 0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419;

    uint256 constant MIN_WETH = 25 ether;
    uint256 constant GAS_UNITS = 1_000_000; // same existing scanner budget
    uint256 constant TIP_WEI = 2 gwei;
    uint256 constant SLIPPAGE_BPS = 10; // scanner's 10 bps
    uint256 constant GAS_BUFFER_PERCENT = 110;
    uint256 constant USD_E8 = 100_000_000;
    uint256 constant HARD_USD5_E8 = 500_000_000;
    struct Reserves {
        address pair;
        uint256 weth;
        uint256 token;
        bool eligible;
    }
    struct Counts {
        uint256 pairsSeen;
        uint256 eligibleMarkets;
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
        uint256 atBlock = vm.envOr("MAINNET_FORK_BLOCK", uint256(0));
        if (atBlock == 0) vm.createSelectFork(rpc);
        else vm.createSelectFork(rpc, atBlock);
        assertEq(block.chainid, 1);
    }

    function _realPair(address factory, address token)
        private view returns (Reserves memory p)
    {
        address pair = IAuditFactory(factory).getPair(WETH, token);
        if (pair == address(0)) return p;
        assertGt(pair.code.length, 0, "factory pair has no code");
        IAuditPair v2 = IAuditPair(pair);
        assertEq(v2.factory(), factory, "factory identity mismatch");
        address t0 = v2.token0();
        address t1 = v2.token1();
        assertTrue((t0 == WETH && t1 == token) || (t1 == WETH && t0 == token),
            "pair token ordering mismatch");
        (uint112 r0, uint112 r1,) = v2.getReserves();
        p.pair = pair;
        p.weth = t0 == WETH ? uint256(r0) : uint256(r1);
        p.token = t0 == WETH ? uint256(r1) : uint256(r0);
        p.eligible = p.weth >= MIN_WETH && p.token > 0;
    }

    function _quote(uint256 amt, uint256 reserveIn, uint256 reserveOut)
        private pure returns (uint256)
    {
        if (amt == 0 || reserveIn == 0 || reserveOut == 0) return 0;
        return amt * 997 * reserveOut / (reserveIn * 1000 + amt * 997);
    }

    function _maxGasQuote() private view returns (uint256 gasWei, uint256 usdE8, uint256 age) {
        IAuditEthUsd oracle = IAuditEthUsd(CHAINLINK_ETH_USD);
        require(oracle.decimals() == 8, "unexpected ETH/USD decimals");
        (uint80 id, int256 price,, uint256 updated, uint80 answered) = oracle.latestRoundData();
        require(id != 0 && answered >= id && price > 0, "invalid Chainlink round");
        require(updated != 0 && updated <= block.timestamp, "invalid oracle timestamp");
        age = block.timestamp - updated;
        require(age <= 3600, "oracle stale: cannot compare USD tiers");
        gasWei = GAS_UNITS * (2 * block.basefee + TIP_WEI);
        // EXACT integer rounding and +10% buffer of one_shot::maximum_gas_cost_usd_e8
        uint256 num = gasWei * uint256(price) * GAS_BUFFER_PERCENT;
        uint256 denom = 1 ether * 100;
        usdE8 = (num + denom - 1) / denom;
    }

    function _market(Reserves memory uni, Reserves memory sushi, uint256 premiumBps,
        uint256 gasWei, uint256 usdE8, Counts memory counts) private pure
        returns (Counts memory)
    {
        if (uni.pair != address(0)) counts.pairsSeen++;
        if (sushi.pair != address(0)) counts.pairsSeen++;
        if (!uni.eligible || !sushi.eligible) return counts;
        counts.eligibleMarkets++;
        for (uint256 i; i < 4; ++i) {
            uint256 amount = i == 0 ? 0.01 ether :
                i == 1 ? 0.05 ether : i == 2 ? 0.1 ether : 0.25 ether;
            uint256 premium = (amount * premiumBps + 9999) / 10000;
            for (uint256 direction; direction < 2; ++direction) {
                Reserves memory first = direction == 0 ? uni : sushi;
                Reserves memory second = direction == 0 ? sushi : uni;
                uint256 afterFirst = _quote(amount, first.weth, first.token);
                uint256 finalWeth = _quote(afterFirst, second.token, second.weth);
                if (finalWeth <= amount) continue;
                counts.grossPositive++;
                // 10bps output haircut, 5bps current Aave premium,
                // 1m gas worst-case EIP-1559 fee in start asset WETH.
                uint256 buffered = finalWeth * (10000 - SLIPPAGE_BPS) / 10000;
                if (buffered <= amount + premium + gasWei) continue;
                counts.netPositive++;
                if (usdE8 <= 5 * USD_E8) counts.within5++;
                if (usdE8 <= 10 * USD_E8) counts.within10++;
                if (usdE8 <= 15 * USD_E8) counts.within15++;
                if (usdE8 <= 20 * USD_E8) counts.within20++;
            }
        }
        return counts;
    }

    function testFork_additionalV2MarketsAndConservativeGasBudgetTiers() public {
        _fork();
        (uint256 gasWei, uint256 gasUsdE8, uint256 priceAge) = _maxGasQuote();
        uint256 premiumBps = IAuditAave(AAVE_POOL).FLASHLOAN_PREMIUM_TOTAL();
        assertTrue(premiumBps >= 1 && premiumBps <= 1000, "invalid Aave premium");

        // Five ADDITIONAL well-known mainnet tokens beyond current scanner
        // USDC/USDT/DAI/WBTC/LINK/UNI; addresses are not execution approvals.
        address[5] memory tokens = [
            address(bytes20(hex"7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9")), // AAVE
            address(bytes20(hex"5a98fcbea516cf06857215779fd812ca3bef1b32")), // LDO
            address(bytes20(hex"d533a949740bb3306d119cc777fa900ba034cd52")), // CRV
            address(bytes20(hex"95ad61b0a150d79219dcf64e1e6cc01f0b64c4c")), // SHIB
            address(bytes20(hex"6982508145454ce325ddbe47a25d4ec3d2311933"))  // PEPE
        ];
        Counts memory total;
        for (uint256 i; i < tokens.length; ++i) {
            Reserves memory uni = _realPair(UNI_FACTORY, tokens[i]);
            Reserves memory sushi = _realPair(SUSHI_FACTORY, tokens[i]);
            total = _market(uni, sushi, premiumBps, gasWei, gasUsdE8, total);
            emit log_named_address("token_under_audit", tokens[i]);
            emit log_named_address("uniswap_pair_or_zero", uni.pair);
            emit log_named_uint("uniswap_weth_reserve_wei", uni.weth);
            emit log_named_uint("uniswap_eligible", uni.eligible ? 1 : 0);
            emit log_named_address("sushiswap_pair_or_zero", sushi.pair);
            emit log_named_uint("sushi_weth_reserve_wei", sushi.weth);
            emit log_named_uint("sushiswap_eligible", sushi.eligible ? 1 : 0);
        }
        emit log_named_uint("fork_block", block.number);
        emit log_named_uint("chainlink_quote_age_sec", priceAge);
        emit log_named_uint("conservative_max_fee_gas_wei", gasWei);
        emit log_named_uint("conservative_max_fee_usd_e8", gasUsdE8);
        emit log_named_uint("within_current_usd5_cap", gasUsdE8 <= HARD_USD5_E8 ? 1 : 0);
        emit log_named_uint("verified_factory_pairs_seen", total.pairsSeen);
        emit log_named_uint("markets_with_two_eligible_pairs", total.eligibleMarkets);
        emit log_named_uint("gross_positive_probe_directions", total.grossPositive);
        emit log_named_uint("estimated_net_positive_probe_directions", total.netPositive);
        emit log_named_uint("estimated_net_within_5_usd", total.within5);
        emit log_named_uint("estimated_net_within_10_usd", total.within10);
        emit log_named_uint("estimated_net_within_15_usd", total.within15);
        emit log_named_uint("estimated_net_within_20_usd", total.within20);
        emit log_string("OBSERVATIONAL ONLY: no execution simulation, USDT approval, signer or trades");
        assertLe(total.within5, total.within10);
        assertLe(total.within10, total.within15);
        assertLe(total.within15, total.within20);
        assertLe(total.within20, total.netPositive);
    }
}
