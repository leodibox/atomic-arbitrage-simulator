import test from "node:test";
import assert from "node:assert/strict";
import {TOKENS,VENUES,factoryData,quoteData,compare} from "./cross_venue_quotes.mjs";
const word=x=>BigInt(x).toString(16).padStart(64,"0");
const output=x=>"0x"+word(x);
const four=(amount)=>"0x"+[amount,1n,0n,100000n].map(word).join("");
test("configured fees and Base addresses",()=>{
 assert.equal(VENUES.uniswap.fee,500n);
 assert.equal(VENUES.pancake.fee,100n);
 assert.equal(factoryData(VENUES.pancake).slice(0,10),"0x1698ee82");
 assert.equal(quoteData(VENUES.pancake,TOKENS.USDC,TOKENS.WETH,100n).slice(0,10),"0xc6a5026a");
});
test("wrong chain stops before pool reads",async()=>{
 const calls=[];
 const fake=async(_url,opt)=>{const q=JSON.parse(opt.body);calls.push(q.method);return{ok:true,json:async()=>({result:"0x1"})};};
 await assert.rejects(compare(fake),/Wrong chain/);
 assert.deepEqual(calls,["eth_chainId"]);
});
test("rejects canonical factory mismatch",async()=>{
 const methods=[];
 const fake=async(_url,opt)=>{
  const q=JSON.parse(opt.body);methods.push(q.method);
  return{ok:true,json:async()=>({result:q.method==="eth_chainId"?"0x2105":q.method==="eth_blockNumber"?"0x10":output(0n)})};
 };
 await assert.rejects(compare(fake),/canonical factory pool mismatch/);
 assert.deepEqual(methods,["eth_chainId","eth_blockNumber","eth_call"]);
});
test("same-block roundtrip quotes in both directions are not net-profit claims",async()=>{
 const calls=[];
 const fake=async(_url,opt)=>{
  const q=JSON.parse(opt.body);calls.push(q);
  let result;
  if(q.method==="eth_chainId")result="0x2105";
  else if(q.method==="eth_blockNumber")result="0xff";
  else if(q.params[0].to===VENUES.uniswap.factory)result=output(BigInt(VENUES.uniswap.pool));
  else if(q.params[0].to===VENUES.pancake.factory)result=output(BigInt(VENUES.pancake.pool));
  else if([VENUES.uniswap.quoter,VENUES.pancake.quoter].includes(q.params[0].to)){
   const isUSDCIn=q.params[0].data.slice(10,74).toLowerCase().endsWith(TOKENS.USDC.slice(2).toLowerCase());
   result=four(isUSDCIn?50000000000000000n:200000000n);
  }else throw Error("unexpected call");
  return{ok:true,json:async()=>({result})};
 };
 const out=await compare(fake);
 assert.equal(out.rpcCalls,16);
 assert.equal(out.results.length,6);
 assert.equal(out.profitVerified,false);assert.equal(out.txSubmitted,0);
 assert(calls.filter(c=>c.method==="eth_call").every(c=>c.params[1]==="0xff"));
 assert(!calls.some(c=>/eth_send|personal_|debug_|trace_/.test(c.method)));
});

test("all configured contract addresses are 20 bytes",()=>{
 for(const [name,venue] of Object.entries(VENUES)){
  for(const field of ["factory","quoter","pool"])
   assert.match(venue[field],/^0x[0-9a-f]{40}$/i,name+" "+field);
 }
});
