// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;
import {Test} from "forge-std/Test.sol";
interface TokenLT { function balanceOf(address) external view returns(uint256); function allowance(address,address)external view returns(uint256); function transfer(address,uint256) external returns(bool); function approve(address,uint256) external returns(bool); }
interface MorphoLT { function flashLoan(address,uint256,bytes calldata) external; }
interface PoolLT {
 function token0() external view returns(address);function token1() external view returns(address);function factory() external view returns(address);function fee() external view returns(uint24);
 function getReserves() external view returns(uint112,uint112,uint32);
 function swap(uint256,uint256,address,bytes calldata) external;
 function swap(address,bool,int256,uint160,bytes calldata) external returns(int256,int256);
}
interface FactoryLT {function getPool(address,address,uint24)external view returns(address);function getPair(address,address)external view returns(address);}
/// Test-only, bounded settlement probe. No production entrypoint or deployment script.
contract LongTailSettlementTest is Test {
 address constant WETH=0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2;
 address constant MORPHO=0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb;
 address constant UNI=0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f;
 address constant SUSHI=0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac;
 address constant V3=0x1F98431c8aD98523631AE4a59f267346ea31F984;
 address first;address second;address other;address active;
 bool firstV3;bool secondV3;bool borrowing;uint256 expected;uint256 owedLimit;
 error Settlement(int256 margin,uint256 gasUsed);
 error TokenTransferMismatch();error Unauthorized();
 function test_candidate0()public{_case(0);}
 function test_candidate1()public{_case(1);}
 function test_candidate2()public{_case(2);}
 function _case(uint256 i)private{
  string memory json=vm.readFile(vm.envString("LONGTAIL_CASES_JSON"));
  bytes memory encoded=vm.parseJson(json); // read only, parser ensures valid JSON
  require(encoded.length>0);
  string memory prefix=string.concat("[",vm.toString(i),"]");
  if(!vm.keyExistsJson(json,string.concat(prefix,".first.address"))){vm.skip(true);return;}
  uint256 blockNumber=vm.parseJsonUint(json,string.concat(prefix,".blockNumber"));
  vm.createSelectFork(vm.envString("HTTPS_URL"),blockNumber);assertEq(block.chainid,1);
  first=vm.parseJsonAddress(json,string.concat(prefix,".first.address"));second=vm.parseJsonAddress(json,string.concat(prefix,".second.address"));
  assertTrue(first!=second,"duplicate pool");other=vm.parseJsonAddress(json,string.concat(prefix,".other"));
  firstV3=keccak256(bytes(vm.parseJsonString(json,string.concat(prefix,".first.kind"))))==keccak256("v3");
  secondV3=keccak256(bytes(vm.parseJsonString(json,string.concat(prefix,".second.kind"))))==keccak256("v3");
  uint256 amount=vm.parseUint(vm.parseJsonString(json,string.concat(prefix,".amountWei")));
  uint256 output=vm.parseUint(vm.parseJsonString(json,string.concat(prefix,".expectedOutputWei")));
  require(amount>0&&amount<=3 ether,"bounded research amount");
  uint256 cash=TokenLT(WETH).balanceOf(MORPHO);assertGe(cash,amount);
  uint256 originalWeth=TokenLT(WETH).balanceOf(address(this));uint256 originalOther=TokenLT(other).balanceOf(address(this));
  expected=amount;borrowing=true;
  uint256 started=gasleft();
  try MorphoLT(MORPHO).flashLoan(WETH,amount,""){
   int256 margin=int256(TokenLT(WETH).balanceOf(address(this)))-int256(originalWeth);
   assertGe(margin,0);assertEq(TokenLT(WETH).balanceOf(MORPHO),cash);
   emit log_named_int("settled_margin_WETH_wei",margin);
  }catch(bytes memory reason){
   assertEq(bytes4(reason),Settlement.selector,"unexpected token/transfer/callback/funding failure");
   int256 margin;uint256 used;
   assembly{margin:=mload(add(reason,36)) used:=mload(add(reason,68))}
   assertEq(margin,int256(output)-int256(amount),"reference quote/settlement mismatch");
   assertEq(TokenLT(WETH).balanceOf(MORPHO),cash);assertEq(TokenLT(WETH).balanceOf(address(this)),originalWeth);
   assertEq(TokenLT(other).balanceOf(address(this)),originalOther);assertEq(TokenLT(WETH).allowance(address(this),MORPHO),0);
   emit log_named_int("observed_margin_WETH_wei",margin);emit log_named_uint("callback_gas_warm",used);
  }
  emit log_named_uint("probe_call_gas_warm",started-gasleft());emit log_named_address("pool_first",first);emit log_named_address("pool_second",second);
 }
 function onMorphoFlashLoan(uint256 amount,bytes calldata)external{
  if(msg.sender!=MORPHO||!borrowing||amount!=expected)revert Unauthorized();
  uint256 start=gasleft();uint256 mid=_swap(first,firstV3,WETH,amount);uint256 end=_swap(second,secondV3,other,mid);
  int256 margin=int256(end)-int256(amount);
  // Always revert this research probe, even if positive. Never pretend repayment completed on a loss.
  revert Settlement(margin,start-gasleft());
 }
 function _transfer(address token,address to,uint256 amount)private{
  uint256 beforeBalance=TokenLT(token).balanceOf(to);
  (bool ok,bytes memory data)=token.call(abi.encodeWithSelector(TokenLT.transfer.selector,to,amount));
  if(!ok||(data.length!=0&&(data.length!=32||!abi.decode(data,(bool))))||TokenLT(token).balanceOf(to)!=beforeBalance+amount)revert TokenTransferMismatch();
 }
 function _swap(address pool,bool v3,address input,uint256 amount)private returns(uint256 out){
  PoolLT p=PoolLT(pool);bool z=input==p.token0();require(z||input==p.token1());address output=z?p.token1():p.token0();uint256 beforeOut=TokenLT(output).balanceOf(address(this));
  if(v3){require(p.factory()==V3&&FactoryLT(V3).getPool(p.token0(),p.token1(),p.fee())==pool);active=pool;owedLimit=amount;
   (int256 a0,int256 a1)=p.swap(address(this),z,int256(amount),z?4295128740:1461446703485210103287273052203988822378723970341,"");
   require(uint256(z?a0:a1)==amount,"partial fill");out=uint256(-(z?a1:a0));active=address(0);
  }else{address fac=p.factory();require(fac==UNI||fac==SUSHI);require(FactoryLT(fac).getPair(p.token0(),p.token1())==pool);
   (uint112 r0,uint112 r1,)=p.getReserves();(uint256 ri,uint256 ro)=z?(uint256(r0),uint256(r1)):(uint256(r1),uint256(r0));
   out=amount*997*ro/(ri*1000+amount*997);_transfer(input,pool,amount);p.swap(z?0:out,z?out:0,address(this),"");
  }
  if(TokenLT(output).balanceOf(address(this))!=beforeOut+out)revert TokenTransferMismatch();
 }
 function uniswapV3SwapCallback(int256 a0,int256 a1,bytes calldata)external{
  if(msg.sender!=active||active==address(0))revert Unauthorized();PoolLT p=PoolLT(msg.sender);
  require(FactoryLT(V3).getPool(p.token0(),p.token1(),p.fee())==msg.sender);
  require((a0>0&&a1<=0)||(a1>0&&a0<=0));uint256 amount=uint256(a0>0?a0:a1);require(amount<=owedLimit);owedLimit=0;
  _transfer(a0>0?p.token0():p.token1(),msg.sender,amount);
 }
}
