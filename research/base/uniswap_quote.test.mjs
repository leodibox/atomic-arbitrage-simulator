import test from "node:test";
import assert from "node:assert/strict";
import {CHAIN_ID,FACTORY,POOL,QUOTER_V2,WETH,USDC,encodeAddress,encodeUint,decodeWords,factoryCallData,quoterCallData,queryUniswap} from "./uniswap_quote.mjs";
const word=v=>"0x"+encodeUint(v);
test("ABI selectors and strict encoding",()=>{
 assert.equal(factoryCallData().slice(0,10),"0x1698ee82");
 assert.equal(quoterCallData(WETH,USDC,100n).slice(0,10),"0xc6a5026a");
 assert.equal(decodeWords(word(33),1)[0],33n);
 assert.throws(()=>decodeWords("0x1234",1));
 assert.throws(()=>encodeAddress("0x1"));
});
test("fail closed on wrong chain",async()=>{
 const calls=[];
 const fake=async(_u,o)=>{const q=JSON.parse(o.body);calls.push(q.method);return{ok:true,json:async()=>({result:"0x1"})};};
 await assert.rejects(queryUniswap(fake),/Wrong chain ID/);
 assert.deepEqual(calls,["eth_chainId"]);
});
test("fail closed if canonical factory rejects pool",async()=>{
 const calls=[];
 const fake=async(_u,o)=>{
  const q=JSON.parse(o.body);calls.push(q.method);
  return{ok:true,json:async()=>({result:q.method==="eth_chainId"?CHAIN_ID:q.method==="eth_blockNumber"?"0xab":word(0n)})};
 };
 await assert.rejects(queryUniswap(fake),/Canonical Uniswap factory/);
 assert.deepEqual(calls,["eth_chainId","eth_blockNumber","eth_call"]);
});
test("six same-block QuoterV2 mock reads with no submissions",async()=>{
 const calls=[];
 const fake=async(_u,o)=>{
  const q=JSON.parse(o.body);calls.push(q);
  const result=q.method==="eth_chainId"?CHAIN_ID:q.method==="eth_blockNumber"?"0xab":
   q.params[0].to===FACTORY?word(BigInt(POOL)):
   "0x"+encodeUint(123n)+encodeUint(999n)+encodeUint(1n)+encodeUint(120000n);
  return{ok:true,json:async()=>({result})};
 };
 const out=await queryUniswap(fake);
 assert.equal(out.quotes.length,6);assert.equal(out.rpcCalls,9);
 assert.equal(out.netProfitVerified,false);assert.equal(out.txSent,0);
 assert.equal(calls.filter(c=>c.method==="eth_call"&&c.params[0].to===QUOTER_V2).length,6);
 assert(calls.filter(c=>c.method==="eth_call").every(c=>c.params[1]==="0xab"));
 assert(!calls.some(c=>c.method.startsWith("eth_send")));
});