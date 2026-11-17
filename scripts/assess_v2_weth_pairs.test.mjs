import test from "node:test";
import assert from "node:assert/strict";
import {ADDRESSES,v2Quote,evaluateWethRoute,assessMarket,getSnapshot,assessSnapshot} from "./assess_v2_weth_pairs.mjs";
const WEI=10n**18n, UNI={wethReserve:1000n*WEI,otherReserve:1800000000000n};
const SUSHI={...UNI};
const word=x=>BigInt(x).toString(16).padStart(64,"0");
const raw=(...x)=>"0x"+x.map(word).join("");
const PAIRS={
 uniUsdt:"0x1111111111111111111111111111111111111111",
 sushiUsdt:"0x2222222222222222222222222222222222222222",
 uniDai:"0x3333333333333333333333333333333333333333",
 sushiDai:"0x4444444444444444444444444444444444444444"
};
function mockRpc({wrongChain=false,wrongFactory=false,missing=false}={}){
 return async(method,args)=>{
  if(method==="eth_chainId")return wrongChain?"0xaa36a7":"0x1";
  if(method==="eth_getBlockByNumber")return {number:"0x7b",timestamp:"0x6a000000",
    baseFeePerGas:"0x77359400",hash:"0x"+"ab".repeat(32)};
  if(method==="eth_getCode")return "0x60606040";
  if(method!=="eth_call")throw Error("Unexpected method");
  if(args[1]!=="0x7b")throw Error("State reads not pinned");
  const addr=args[0].to.toLowerCase(),sig=args[0].data.slice(0,10);
  if(sig==="0x313ce567")return raw(addr===ADDRESSES.usdt.toLowerCase()?6n:18n);
  if(sig==="0x074b2e43")return raw(5n);
  if(sig==="0xe6a43905"){
   const dai=args[0].data.slice(-40).toLowerCase()===ADDRESSES.dai.slice(2).toLowerCase();
   const uni=addr===ADDRESSES.uniFactory.toLowerCase();
   return raw(missing?0n:BigInt(uni?(dai?PAIRS.uniDai:PAIRS.uniUsdt):(dai?PAIRS.sushiDai:PAIRS.sushiUsdt)));
  }
  if(sig==="0xc45a0155"){
   const uni=[PAIRS.uniDai.toLowerCase(),PAIRS.uniUsdt.toLowerCase()].includes(addr);
   return raw(BigInt(wrongFactory?ADDRESSES.aavePool:
    uni?ADDRESSES.uniFactory:ADDRESSES.sushiFactory));
  }
  if(sig==="0x0dfe1681")return raw(BigInt(ADDRESSES.weth));
  if(sig==="0xd21220a7")return raw(BigInt(
    [PAIRS.uniDai.toLowerCase(),PAIRS.sushiDai.toLowerCase()].includes(addr)?
     ADDRESSES.dai:ADDRESSES.usdt));
  if(sig==="0x0902f1ac")return raw(1000n*WEI,1800000000000n,1n);
  throw Error("Unexpected selector");
 };
}
test("equal V2 reserve quotes lose money to two 30bps fees",()=>{
 const mid=v2Quote(WEI,UNI.wethReserve,UNI.otherReserve);
 assert.ok(v2Quote(mid,SUSHI.otherReserve,SUSHI.wethReserve)<WEI);
});
test("gas and Aave premium deducted in WETH wei",()=>{
 const a=evaluateWethRoute(UNI,SUSHI,WEI,5n,3n*10n**15n);
 assert.equal(a.premiumWei,500000000000000n);
 assert.equal(a.estimatedPostGasWei,a.marginBeforeGasWei-3000000000000000n);
 assert.equal(a.approvedForExecution,false);
});
test("even synthetic positive quote never approves execution",()=>{
 const first={wethReserve:1000n*WEI,otherReserve:2000000n*WEI};
 const second={wethReserve:1000n*WEI,otherReserve:1000000n*WEI};
 const a=evaluateWethRoute(first,second,WEI,5n,1000000000000000n);
 assert.equal(a.theoreticalPositive,true);
 assert.equal(a.approvedForExecution,false);
});
test("missing factory pair emits unavailable not fabricated opportunities",()=>{
 const x=assessMarket({token:"DAI",missing:true,reason:"zero address"},{baseFeeWei:100n,premiumBps:5n});
 assert.equal(x.status,"UNAVAILABLE");assert.equal(x.routes.length,0);
});
test("10 sizes and directions per market, never a trade signal",()=>{
 const a=assessSnapshot({blockNumber:123n,blockHash:"0xabc",baseFeeWei:100n,
  premiumBps:5n,markets:[{token:"USDT",uni:UNI,sushi:SUSHI,missing:false}]});
 assert.equal(a.markets[0].routes.length,10);
 assert.ok(a.markets[0].routes.every(r=>r.approvedForExecution===false));
 assert.equal(a.assumptions.transactionSubmissionEnabled,false);
});
test("requires chain ID 1",async()=>{
 await assert.rejects(()=>getSnapshot(mockRpc({wrongChain:true})),/Wrong chain/);
});
test("pinned RPC reads discover two canonical markets and true token decimals",async()=>{
 const snap=await getSnapshot(mockRpc());
 assert.equal(snap.blockNumber,123n);
 assert.deepEqual(snap.markets.map(m=>[m.token,m.decimals]),[["USDT",6],["DAI",18]]);
 assert.ok(snap.markets.every(m=>!m.missing));
 assert.ok(assessSnapshot(snap).markets.every(m=>m.routes.length===10));
});
test("reject mismatched V2 factory",async()=>{
 await assert.rejects(()=>getSnapshot(mockRpc({wrongFactory:true})),/Pair\/factory identity mismatch/);
});
test("missing pair remains unavailable for both markets",async()=>{
 const s=await getSnapshot(mockRpc({missing:true}));
 assert.ok(assessSnapshot(s).markets.every(m=>m.status==="UNAVAILABLE"));
});
test("fail closed for invalid block and wrong historical block",async()=>{
 await assert.rejects(()=>getSnapshot(mockRpc(),{blockTag:"0x7c"}),/different block/);
 await assert.rejects(()=>getSnapshot(mockRpc(),{blockTag:"random-url"}),/Invalid block selector/);
});
