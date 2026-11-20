// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;
import {Test} from "forge-std/Test.sol";
import {ResearchReceiver, TokenR, PoolR, FactoryR, PairR} from "../src/ResearchReceiver.sol";
interface QuoterR {
    struct Params { address tokenIn; address tokenOut; uint256 amountIn; uint24 fee; uint160 sqrtPriceLimitX96; }
    function quoteExactInputSingle(Params calldata) external returns (uint256, uint160, uint32, uint256);
}
/// Forged self-reported getters must not bypass canonical factory registration.
contract PoolImpostor {
    function factory() external pure returns(address) { return 0x1F98431c8aD98523631AE4a59f267346ea31F984; }
    function token0() external pure returns(address) { return 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48; }
    function token1() external pure returns(address) { return 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2; }
    function fee() external pure returns(uint24) { return 500; }
}
contract MorphoV3MainnetCertificationTest is Test {
    address constant MORPHO = 0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb;
    address constant WETH = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
    address constant USDC = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48;
    address constant FACTORY = 0x1F98431c8aD98523631AE4a59f267346ea31F984;
    address constant QUOTER = 0x61fFE014bA17989E743c5F6cB21bF9697530B21e;
    address constant V2 = 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc;
    ResearchReceiver receiver;
    address p5; address p1;
    function setUp() public {
        vm.createSelectFork(vm.envString("HTTPS_URL"),vm.envUint("CERT_FORK_BLOCK"));
        assertEq(block.chainid,1);
        assertGt(MORPHO.code.length,0);assertGt(FACTORY.code.length,0);assertGt(QUOTER.code.length,0);
        p5=FactoryR(FACTORY).getPool(USDC,WETH,500);p1=FactoryR(FACTORY).getPool(USDC,WETH,100);
        receiver=new ResearchReceiver();
    }
    function _loan(address token,uint256 amount) private {
        uint256 cash=TokenR(token).balanceOf(MORPHO);assertGe(cash,amount);
        vm.expectCall(token,abi.encodeWithSignature("transfer(address,uint256)",address(receiver),amount));
        vm.expectCall(token,abi.encodeWithSignature("transferFrom(address,address,uint256)",address(receiver),MORPHO,amount));
        uint256 start=gasleft();receiver.borrow(token,amount,0);
        emit log_named_address("loan_asset",token);emit log_named_uint("principal_raw",amount);
        emit log_named_uint("loan_gas_warm_local",start-gasleft());
        assertEq(receiver.callbackBalance(),amount);
        assertEq(TokenR(token).balanceOf(MORPHO),cash,"nonzero fee or missing principal");
        assertEq(TokenR(token).balanceOf(address(receiver)),0);
        assertEq(TokenR(token).allowance(address(receiver),MORPHO),0);
    }
    function testMorpho_WETH_001() public { _loan(WETH,0.01 ether); }
    function testMorpho_WETH_1() public { _loan(WETH,1 ether); }
    function testMorpho_WETH_10() public { _loan(WETH,10 ether); }
    function testMorpho_USDC_100() public { _loan(USDC,100e6); }
    function testMorpho_USDC_10000() public { _loan(USDC,10_000e6); }
    function testMorpho_USDC_1000000() public { _loan(USDC,1_000_000e6); }
    function _failedLoan(address token,uint8 mode) private {
        uint256 cash=TokenR(token).balanceOf(MORPHO);
        if(mode==1)vm.expectRevert(ResearchReceiver.IntentionalCallbackFailure.selector);else vm.expectRevert();
        receiver.borrow(token,token==WETH?1 ether:100e6,mode);
        assertEq(TokenR(token).balanceOf(MORPHO),cash);assertEq(TokenR(token).balanceOf(address(receiver)),0);
        assertEq(receiver.callbackBalance(),0);assertEq(TokenR(token).allowance(address(receiver),MORPHO),0);
        receiver.borrow(token,token==WETH?1 ether:100e6,0);
    }
    function testMorpho_callbackFailureAtomicWeth() public { _failedLoan(WETH,1); }
    function testMorpho_callbackFailureAtomicUsdc() public { _failedLoan(USDC,1); }
    function testMorpho_repaymentFailureAtomicWeth() public { _failedLoan(WETH,2); }
    function testMorpho_repaymentFailureAtomicUsdc() public { _failedLoan(USDC,2); }
    function testMorpho_excessCashRejected() public {
        uint256 cash=TokenR(WETH).balanceOf(MORPHO);vm.expectRevert();receiver.borrow(WETH,cash+1,0);
        assertEq(TokenR(WETH).balanceOf(MORPHO),cash);assertEq(TokenR(WETH).balanceOf(address(receiver)),0);
    }
    function testMorpho_forgedAndInactiveCallbacksRejected() public {
        vm.expectRevert(ResearchReceiver.Unauthorized.selector);receiver.onMorphoFlashLoan(1,"");
        vm.prank(MORPHO);vm.expectRevert(ResearchReceiver.Inactive.selector);receiver.onMorphoFlashLoan(1,"");
    }
    function testReceiver_ownerAuthentication() public {
        vm.startPrank(address(0xBAD));vm.expectRevert(ResearchReceiver.Unauthorized.selector);receiver.borrow(WETH,1 ether,0);
        vm.expectRevert(ResearchReceiver.Unauthorized.selector);receiver.swapExactIn(p5,false,1,0,0,false);vm.stopPrank();
    }
    function _identity(address p,uint24 fee) private view {
        assertTrue(p!=address(0));assertGt(p.code.length,0);assertEq(PoolR(p).factory(),FACTORY);
        assertEq(PoolR(p).token0(),USDC);assertEq(PoolR(p).token1(),WETH);
        assertEq(PoolR(p).fee(),fee);assertGt(PoolR(p).liquidity(),0);
    }
    function testV3_canonical5bpIdentity() public view { _identity(p5,500); }
    function testV3_canonical1bpIdentity() public view { _identity(p1,100); }
    function _fundWeth(uint256 amount) private {
        // Synthetic wallet funding only, never reserve/price manipulation.
        vm.deal(address(this),amount);TokenR(WETH).deposit{value:amount}();
        require(TokenR(WETH).transfer(address(receiver),amount));
    }
    function _quote(address p,address input,uint256 amount) private returns(uint256 out,uint160 sqrtAfter,uint32 crossed) {
        (out,sqrtAfter,crossed,)=QuoterR(QUOTER).quoteExactInputSingle(
            QuoterR.Params(input,input==USDC?WETH:USDC,amount,PoolR(p).fee(),0));
    }
    struct SwapState { uint256 quote; uint160 beforeSqrt; uint160 expectedSqrt; uint32 crossed; uint256 beforeIn; uint256 beforeOut; uint256 poolIn; uint256 poolOut; uint256 start; uint256 used; }
    function _swapAndCheck(address p,address input,uint256 amount) private returns(uint32) {
        SwapState memory s;bool z=input==USDC;address output=z?WETH:USDC;
        (s.beforeSqrt,,,,,,)=PoolR(p).slot0();
        if(input==WETH)_fundWeth(amount);else deal(USDC,address(receiver),amount);
        (s.quote,s.expectedSqrt,s.crossed)=_quote(p,input,amount);
        s.beforeIn=TokenR(input).balanceOf(address(receiver));s.beforeOut=TokenR(output).balanceOf(address(receiver));
        s.poolIn=TokenR(input).balanceOf(p);s.poolOut=TokenR(output).balanceOf(p);
        vm.expectCall(input,abi.encodeWithSignature("transfer(address,uint256)",p,amount));
        s.start=gasleft();uint256 out=receiver.swapExactIn(p,z,amount,s.quote,0,false);s.used=s.start-gasleft();
        assertEq(out,s.quote,"QuoterV2 exact output mismatch");
        assertEq(TokenR(input).balanceOf(address(receiver)),s.beforeIn-amount);
        assertEq(TokenR(output).balanceOf(address(receiver)),s.beforeOut+out);
        assertEq(TokenR(input).balanceOf(p),s.poolIn+amount);assertEq(TokenR(output).balanceOf(p),s.poolOut-out);
        (uint160 sqrtAfter,,,,,,)=PoolR(p).slot0();assertEq(sqrtAfter,s.expectedSqrt);
        assertEq(z?receiver.lastAmount0():receiver.lastAmount1(),int256(amount));
        assertEq(z?receiver.lastAmount1():receiver.lastAmount0(),-int256(out));
        emit log_named_address("swap_pool",p);emit log_named_address("input_token",input);
        emit log_named_uint("swap_input_raw",amount);emit log_named_uint("swap_output_raw",out);
        emit log_named_uint("sqrtPriceX96_before",s.beforeSqrt);emit log_named_uint("sqrtPriceX96_after",sqrtAfter);
        emit log_named_uint("initialized_ticks_crossed",s.crossed);emit log_named_uint("swap_gas_warm_local",s.used);
        return s.crossed;
    }
    function testV3_5bpWethToUsdcExactOutputAndPayment() public { _swapAndCheck(p5,WETH,0.01 ether); }
    function testV3_5bpUsdcToWethExactOutputAndPayment() public { _swapAndCheck(p5,USDC,100e6); }
    function testV3_1bpWethToUsdcExactOutputAndPayment() public { _swapAndCheck(p1,WETH,0.01 ether); }
    function testV3_1bpUsdcToWethExactOutputAndPayment() public { _swapAndCheck(p1,USDC,100e6); }
    function testV3_initializedTickCrossing() public { uint128 beforeL=PoolR(p5).liquidity();
        assertGt(_swapAndCheck(p5,WETH,100 ether),0,"no initialized tick crossed");
        assertTrue(PoolR(p5).liquidity()!=beforeL,"no actual liquidity transition"); }
    function _rollbackSwap(uint8 failure) private {
        _fundWeth(1 ether);(uint160 beforeSqrt,,,,,,)=PoolR(p5).slot0();
        uint256 pw=TokenR(WETH).balanceOf(p5);uint256 pu=TokenR(USDC).balanceOf(p5);
        if(failure==1)vm.expectRevert(ResearchReceiver.Slippage.selector);
        else if(failure==2)vm.expectRevert(bytes("IIA"));else vm.expectRevert(ResearchReceiver.PartialFill.selector);
        receiver.swapExactIn(p5,false,1 ether,failure==1?type(uint256).max:0,failure==3?beforeSqrt+1:0,failure==2);
        (uint160 afterSqrt,,,,,,)=PoolR(p5).slot0();assertEq(afterSqrt,beforeSqrt);
        assertEq(TokenR(WETH).balanceOf(p5),pw);assertEq(TokenR(USDC).balanceOf(p5),pu);
        assertEq(TokenR(WETH).balanceOf(address(receiver)),1 ether);assertEq(TokenR(USDC).balanceOf(address(receiver)),0);
    }
    function testV3_slippageFailureRollsBack() public { _rollbackSwap(1); }
    function testV3_underpaymentRollsBack() public { _rollbackSwap(2); }
    function testV3_insufficientLiquidityWithinPriceLimitRollsBack() public { _rollbackSwap(3); }
    function testV3_forgedAndInactiveCallbacksRejected() public {
        vm.expectRevert(ResearchReceiver.Unauthorized.selector);receiver.uniswapV3SwapCallback(1,-1,"");
        vm.prank(p5);vm.expectRevert(ResearchReceiver.Unauthorized.selector);receiver.uniswapV3SwapCallback(1,-1,"");
    }
    function testV3_fabricatedCanonicalGettersRejected() public {
        PoolImpostor fake=new PoolImpostor();
        vm.expectRevert(ResearchReceiver.BadPool.selector);
        receiver.swapExactIn(address(fake),false,1,0,0,false);
    }
    function testV3_nonCanonicalPoolRejected() public { vm.expectRevert();receiver.swapExactIn(V2,false,1,0,0,false); }
    function testV3_exactSingleRangeFee5bp() public { _exactFee(p5); }
    function testV3_exactSingleRangeFee1bp() public { _exactFee(p1); }
    struct FeeState { uint160 beforeSqrt; int24 tickBefore; uint8 protocol; uint128 liquidity; uint256 growth; }
    function _exactFee(address p) private {
        FeeState memory s;
        (s.beforeSqrt,s.tickBefore,,,,s.protocol,)=PoolR(p).slot0();s.liquidity=PoolR(p).liquidity();s.growth=PoolR(p).feeGrowthGlobal1X128();
        _swapAndCheck(p,WETH,0.000001 ether);
        assertEq(PoolR(p).liquidity(),s.liquidity,"single-range fee changed liquidity");
        (uint160 afterSqrt,int24 tickAfter,,,,,)=PoolR(p).slot0();int24 spacing=PoolR(p).tickSpacing();
        assertEq((s.tickBefore/spacing)>>8,(tickAfter/spacing)>>8,"fee test crossed bitmap word");
        uint256 net=(uint256(s.liquidity)*(afterSqrt-s.beforeSqrt)+(1<<96)-1)/(1<<96);
        uint256 exactFee=uint256(0.000001 ether)-net;uint256 protocolPart=s.protocol>>4;
        uint256 lpFee=exactFee-(protocolPart==0?0:exactFee/protocolPart);
        assertEq(PoolR(p).feeGrowthGlobal1X128()-s.growth,(lpFee*(1<<128))/s.liquidity,"wrong actual fee growth");
        uint256 minimumFee=(uint256(0.000001 ether)*uint256(PoolR(p).fee())+999999)/1_000_000;
        assertGe(exactFee,minimumFee);assertLe(exactFee,minimumFee+1);
        emit log_named_uint("exact_fee_WETH_wei",exactFee);
    }
    function _economy(address first,address second,bool v2) private {
        uint256 amount=0.1 ether;(uint256 intermediate,,)=_quote(first,WETH,amount);uint256 out;
        if(v2){(uint112 r0,uint112 r1,)=PairR(second).getReserves();
            assertEq(PairR(second).token0(),USDC);assertEq(PairR(second).token1(),WETH);
            out=intermediate*997*uint256(r1)/(uint256(r0)*1000+intermediate*997);
        }else(out,,)=_quote(second,USDC,intermediate);
        int256 margin=int256(out)-int256(amount);uint256 cash=TokenR(WETH).balanceOf(MORPHO);
        uint256 start=gasleft();
        try receiver.roundTrip(WETH,amount,first,second,v2) {
            assertGe(receiver.routeMargin(),0);
            margin=receiver.routeMargin();
        } catch(bytes memory reason) {
            assertEq(bytes4(reason),ResearchReceiver.Unprofitable.selector,"unexpected route failure");
            int256 realized;
            assembly { realized := mload(add(reason,36)) }
            assertLt(realized,0);
            // Same-pool return quotes must use the changed state, not the original state.
            if(first!=second)assertEq(realized,margin,"disjoint quote/execution disagreement");
            margin=realized;
        }
        uint256 used=start-gasleft();
        assertEq(TokenR(WETH).balanceOf(MORPHO),cash);
        if(margin<0)assertEq(TokenR(WETH).balanceOf(address(receiver)),0);
        emit log_named_int("route_margin_before_gas_WETH_wei",margin);
        emit log_named_uint("roundtrip_gas_warm_local",used);emit log_named_uint("roundtrip_input_WETH_wei",amount);
        emit log_named_address("first_pool",first);emit log_named_address("second_pool",second);
    }
    function testEconomy_5bpThenV2Morpho() public { _economy(p5,V2,true); }
    function testEconomy_5bpThenSame5bpMorpho() public { _economy(p5,p5,false); }
    function testEconomy_1bpThenSame1bpMorpho() public { _economy(p1,p1,false); }
    function testEconomy_5bpThen1bpMorpho() public { _economy(p5,p1,false); }
    function testEconomy_1bpThen5bpMorpho() public { _economy(p1,p5,false); }
}
