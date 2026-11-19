import test from "node:test";
import assert from "node:assert/strict";
import {EXPECTED,decodeAddress,decodeUint,validateMetadata,inspect} from "./verify_pools.mjs";
const addr=v=>"0x"+"0".repeat(24)+v.slice(2).toLowerCase();
const word=v=>"0x"+BigInt(v).toString(16).padStart(64,"0");
test("strict word decoder and chain id",()=>{
  assert.equal(decodeAddress(addr(EXPECTED.weth)),EXPECTED.weth.toLowerCase());
  assert.equal(decodeUint(word(500)),"500");
  assert.throws(()=>decodeUint("0x01"));
});
test("fail closed on wrong fee, tokens, empty liquidity and code",()=>{
  const good={token0:EXPECTED.weth,token1:EXPECTED.usdc,fee:"500",liquidity:"100",sqrtPriceX96:"99",code:"0x6000"};
  validateMetadata(good);
  for(const delta of [{fee:"3000"},{token1:"0x0000000000000000000000000000000000000000"},{liquidity:"0"},{code:"0x"},{sqrtPriceX96:"0"}]) assert.throws(()=>validateMetadata({...good,...delta}));
});
test("refuse non Base chain before any contract reads",async()=>{
 let calls=[];
 const fake=async (_url,opt)=>{const q=JSON.parse(opt.body);calls.push(q.method);return{ok:true,json:async()=>({result:"0x1"})};};
 await assert.rejects(inspect(fake),/not Base mainnet/);
 assert.deepEqual(calls,["eth_chainId"]);
});
test("both pools must pass same-block on-chain reads (mocked)",async()=>{
 const calls=[];
 const selectors={"0x0dfe1681":addr(EXPECTED.usdc),"0xd21220a7":addr(EXPECTED.weth),
   "0xddca3f43":word(500),"0x1a686502":word(10000),
   "0x3850c7bd":word(2n**96n)+"0".repeat(64*6),
   "0xc45a0155":addr("0x00000000000000000000000000000000000000ff")};
 const fake=async (_url,opt)=>{
  const q=JSON.parse(opt.body);calls.push(q);
  let result=q.method==="eth_chainId"?"0x2105":q.method==="eth_blockNumber"?"0xa0":
    q.method==="eth_getCode"?"0x60006000":selectors[q.params[0].data];
  return {ok:true,json:async()=>({result})};
 };
 const out=await inspect(fake);
 assert.equal(out.rpcCalls,16);
 assert.equal(out.pools.length,2);
 assert.equal(out.netProfitVerified,false);
 assert.equal(out.txSent,0);
 assert.equal(calls.filter(c=>c.method==="eth_call").length,12);
 assert(calls.every(c=>!["eth_sendRawTransaction","eth_sendTransaction"].includes(c.method)));
 assert(calls.filter(c=>["eth_call","eth_getCode"].includes(c.method)).every(c=>c.params[1]==="0xa0"));
});
