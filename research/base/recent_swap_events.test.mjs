import test from "node:test";
import assert from "node:assert/strict";
import {POOLS,checkLog,sample} from "./recent_swap_events.mjs";
const block="0x100";
const h="0x"+"a".repeat(64),tx="0x"+"b".repeat(64);
function mk(pool=POOLS[0],n=0x100){
 return {address:pool.address,topics:[pool.topic],removed:false,
 blockNumber:"0x"+n.toString(16),blockHash:h,transactionHash:tx,logIndex:"0x1",data:"0x"};
}
test("venue-specific Swap topics are not conflated",()=>{
 assert.notEqual(POOLS[0].topic,POOLS[1].topic);
 for(const pool of POOLS)assert.match(pool.topic,/^0x[0-9a-f]{64}$/);
});
test("reject wrong topic, wrong pool, removed and out-of-window log",()=>{
 const v=mk();assert.equal(checkLog(v,POOLS[0],0x100n,0x100n),true);
 assert.equal(checkLog({...v,removed:true},POOLS[0],0x100n,0x100n),false);
 assert.equal(checkLog({...v,topics:[POOLS[1].topic]},POOLS[0],0x100n,0x100n),false);
 assert.equal(checkLog({...v,address:POOLS[1].address},POOLS[0],0x100n,0x100n),false);
 assert.equal(checkLog(v,POOLS[0],0x101n,0x102n),false);
});
test("same block windows, bounded 14 RPC calls, zero trades, no made-up net PnL",async()=>{
 const calls=[];
 const fetcher=async(_url,options)=>{
  const q=JSON.parse(options.body);calls.push(q);
  let result;
  if(q.method==="eth_chainId")result="0x2105";
  else if(q.method==="eth_blockNumber")result=block;
  else if(q.method==="eth_getLogs"){
   const p=POOLS.find(p=>p.address===q.params[0].address);
   result=q.params[0].toBlock==="0x100"?[mk(p)]:[];
  }else throw Error("unexpected method");
  return {ok:true,json:async()=>({result})};
 };
 const data=await sample(fetcher);
 assert.equal(data.calls,14);
 assert.equal(data.validEvents,2);
 assert.equal(data.rejectedLogs,0);
 assert.equal(data.netPositiveCount,null);
 assert.equal(data.historicalStateVerified,false);
 assert.equal(data.transactionsSent,0);
 assert(calls.filter(x=>x.method==="eth_getLogs").every(x=>x.params[0].topics.length===1));
 assert(!calls.some(x=>/eth_send|debug_|trace_/.test(x.method)));
});
test("RPC denial fails closed rather than inventing empty history",async()=>{
 const fetcher=async(_u,o)=>{
  const q=JSON.parse(o.body);
  return{ok:true,json:async()=>q.method==="eth_chainId"?{result:"0x2105"}:
   q.method==="eth_blockNumber"?{result:"0x100"}:{error:{code:-32602,message:"invalid params"}}};
 };
 await assert.rejects(sample(fetcher),/getLogs RPC refused/);
});
