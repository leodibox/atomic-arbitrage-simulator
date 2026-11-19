import test from "node:test";
import assert from "node:assert/strict";
import {WETH,USDC,PANCAKE,AERO,inspect,pancakeFactoryData,pancakeQuoteData,aeroQuoteData} from "./aero_pancake_quotes.mjs";
const w=n=>BigInt(n).toString(16).padStart(64,"0");
const uint=n=>"0x"+w(n);
const four=n=>"0x"+[n,2n,0n,150000n].map(w).join("");
test("Aerodrome uses tickSpacing ABI, not Pancake fee ABI",()=>{
 assert.equal(aeroQuoteData(WETH,USDC,123n,1n).slice(0,10),"0x9e7defe6");
 assert.equal(pancakeQuoteData(WETH,USDC,123n).slice(0,10),"0xc6a5026a");
 assert.equal(pancakeFactoryData().slice(0,10),"0x1698ee82");
});
test("malformed networks fail closed before contract reads",async()=>{
 const seen=[];
 const fake=async(_url,args)=>{const v=JSON.parse(args.body);seen.push(v.method);return{ok:true,json:async()=>({result:"0x1"})};};
 await assert.rejects(inspect(fake),/Not Base mainnet/);
 assert.deepEqual(seen,["eth_chainId"]);
});
test("reject canonical Pancake factory mismatch",async()=>{
 const seen=[];
 const fake=async(_url,args)=>{
  const q=JSON.parse(args.body);seen.push(q.method);
  const result=q.method==="eth_chainId"?"0x2105":q.method==="eth_blockNumber"?"0xf0":uint(0);
  return{ok:true,json:async()=>({result})};
 };
 await assert.rejects(inspect(fake),/Pancake factory pool mismatch/);
 assert.equal(seen.length,3);
});
test("six spot roundtrips at same block with no send methods",async()=>{
 const seen=[];
 const fake=async(_url,args)=>{
  const q=JSON.parse(args.body);seen.push(q);
  let result;
  if(q.method==="eth_chainId")result="0x2105";
  else if(q.method==="eth_blockNumber")result="0xa1";
  else if(q.params[0].to===PANCAKE.factory)result=uint(BigInt(PANCAKE.pool));
  else if(q.params[0].to===AERO.pool){
   const selector=q.params[0].data;
   result=uint(selector==="0xc45a0155"?BigInt(AERO.factory):
    selector==="0x0dfe1681"?BigInt(USDC):
    selector==="0xd21220a7"?BigInt(WETH):
    selector==="0xd0c93a7c"?1n:1000000n);
  }else if([PANCAKE.quoter,AERO.quoter].includes(q.params[0].to)){
   const isUSDCIn=q.params[0].data.slice(10,74).toLowerCase().endsWith(USDC.slice(2));
   result=four(isUSDCIn?50000000000000000n:200000000n);
  }else throw Error("Unexpected contract");
  return{ok:true,json:async()=>({result})};
 };
 const out=await inspect(fake);
 assert.equal(out.results.length,6);
 assert.equal(out.rpcCalls,20);
 assert.equal(out.aeroTickSpacing,1);
 assert.equal(out.netProfitVerified,false);
 assert.equal(out.txSubmitted,0);
 assert(seen.filter(v=>v.method==="eth_call").every(v=>v.params[1]==="0xa1"));
 assert(!seen.some(v=>v.method.startsWith("eth_send")));
});
