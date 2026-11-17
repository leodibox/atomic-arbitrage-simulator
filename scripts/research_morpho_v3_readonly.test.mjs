import test from "node:test";
import assert from "node:assert/strict";
import {ADDRESS,feeScenarios,collectReadOnlyEvidence,rpcHttps} from "./research_morpho_v3_readonly.mjs";

const pool500="0x1111111111111111111111111111111111111111";
const pool100="0x2222222222222222222222222222222222222222";
const raw=v=>"0x"+BigInt(v).toString(16).padStart(64,"0");
const matches=(a,b)=>a.toLowerCase()===b.toLowerCase();
function mock({chain="0x1",missing1bp=false,wrongFactory=false,zeroLiquidity=false}={}){
  const calls=[];
  const rpc=async(method,params)=>{
    calls.push({method,params});
    if(method==="eth_chainId")return chain;
    if(method==="eth_getBlockByNumber")return {number:"0x7b",timestamp:"0x6a000000",
      hash:"0x"+"ab".repeat(32)};
    if(method==="eth_getCode")return "0x60006000";
    if(method!=="eth_call")throw Error("Unexpected call");
    if(params[1]!=="0x7b")throw Error("Unpinned read");
    const to=params[0].to, data=params[0].data;
    if(matches(to,ADDRESS.aave)&&data==="0x074b2e43")return raw(5n);
    if([ADDRESS.usdc,ADDRESS.weth].some(x=>matches(x,to))&&data.startsWith("0x70a08231")){
      return raw(matches(to,ADDRESS.weth)?100000000000000000000n:500000000n);
    }
    if(matches(to,ADDRESS.factory)&&data.startsWith("0x1698ee82")){
      const fee=BigInt("0x"+data.slice(-64));
      return raw(missing1bp&&fee===100n?0n:BigInt(fee===100n?pool100:pool500));
    }
    if([pool100,pool500].some(p=>matches(p,to))){
      if(data==="0xc45a0155")return raw(BigInt(wrongFactory?ADDRESS.aave:ADDRESS.factory));
      if(data==="0x0dfe1681")return raw(BigInt(ADDRESS.usdc));
      if(data==="0xd21220a7")return raw(BigInt(ADDRESS.weth));
      if(data==="0xddca3f43")return raw(BigInt(matches(to,pool100)?100:500));
      if(data==="0x1a686502")return raw(zeroLiquidity?0n:100000000000n);
    }
    throw Error("Unsupported ABI call");
  };
  return {rpc,calls};
}
test("cost-only fee math distinguishes 1bp, 5bp and Aave premium",()=>{
  const s=feeScenarios({v3FeeBps:5,aaveFeeBps:5n});
  assert.equal(s.v3ThenV2WithMorphoBps,35n);
  assert.equal(s.v3ThenV2WithAaveBps,40n);
  assert.equal(s.v3ThenV3WithMorphoBps,10n);
  assert.equal(feeScenarios({v3FeeBps:1,aaveFeeBps:5n}).v3ThenV3WithMorphoBps,2n);
  assert.equal(s.approvedForExecution,false);
});
test("snapshot authenticates canonical providers, chain, pools and all pinned reads",async()=>{
 const {rpc,calls}=mock();
 const x=await collectReadOnlyEvidence(rpc);
 assert.equal(x.chainId,1);
 assert.equal(x.blockNumber,123n);
 assert.equal(x.providers.aave.onchainPremiumBps,5n);
 assert.equal(x.providers.morpho.cashBalancesRaw.WETH,100000000000000000000n);
 assert.equal(x.providers.morpho.documentedFlashFeeBps,0);
 assert.equal(x.providers.morpho.feeWasVerifiedFromContract,false);
 assert.deepEqual(x.uniswapV3Pools.map(p=>p.feeBps),[1,5]);
 assert.ok(x.uniswapV3Pools.every(p=>p.status==="IN_RANGE_LIQUIDITY_PRESENT"));
 assert.ok(calls.filter(c=>["eth_call","eth_getCode"].includes(c.method))
   .every(c=>c.params[1]==="0x7b"));
});
test("wrong chain fails closed",async()=>{
 await assert.rejects(()=>collectReadOnlyEvidence(mock({chain:"0xaa36a7"}).rpc),/Wrong chain/);
});
test("wrong factory fails closed",async()=>{
 await assert.rejects(()=>collectReadOnlyEvidence(mock({wrongFactory:true}).rpc),/factory\/pool identity mismatch/);
});
test("missing 1bp pool does not fabricate 1bp price route",async()=>{
 const x=await collectReadOnlyEvidence(mock({missing1bp:true}).rpc);
 assert.equal(x.uniswapV3Pools[0].status,"POOL_MISSING");
 assert.equal(x.uniswapV3Pools[0].approvedForExecution,false);
});
test("zero active liquidity is reported but never approved",async()=>{
 const x=await collectReadOnlyEvidence(mock({zeroLiquidity:true}).rpc);
 assert.ok(x.uniswapV3Pools.every(p=>p.status==="ZERO_IN_RANGE_LIQUIDITY"));
 assert.equal(x.assumptions.netProfitVerified,false);
});
test("wrong fixed historical block fails closed",async()=>{
 await assert.rejects(()=>collectReadOnlyEvidence(mock().rpc,{tag:"0x7c"}),/Mismatched block/);
 await assert.rejects(()=>collectReadOnlyEvidence(mock().rpc,{tag:"bogus"}),/Invalid block tag/);
});
test("HTTP RPC rejects non-HTTPS and sends JSON-RPC POST only",async()=>{
 await assert.rejects(()=>rpcHttps("http://insecure", "eth_call"),/HTTPS_URL/);
 const result=await rpcHttps("https://example.invalid", "eth_chainId",[],
   async(_url,opts)=>({ok:true,json:async()=>JSON.parse(opts.body).method==="eth_chainId"?
     {result:"0x1"}:{error:"unexpected"}}));
 assert.equal(result,"0x1");
});
