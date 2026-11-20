import test from "node:test";
import assert from "node:assert/strict";
import {gasUsdE8,inspectGasBudget,ETH_USD_FEED} from "./verify_gas_usd.mjs";

const hex=(x)=>"0x"+x.toString(16);
const round=(answer=300000000000n,roundId=1n,answered=1n,updated=1000n)=>
  "0x"+[roundId,answer,0n,updated,answered].map(x=>x.toString(16).padStart(64,"0")).join("");
const valid=(overrides={})=>(method,params)=>{
  if(method==="eth_chainId") return Promise.resolve(overrides.chain||"0x1");
  if(method==="eth_getBlockByNumber") return Promise.resolve({timestamp:overrides.ts||hex(1100n)});
  if(method==="eth_call") {
    assert.equal(params[0].to,ETH_USD_FEED);
    if(params[0].data==="0x313ce567") return Promise.resolve(overrides.decimals||"0x8");
    if(params[0].data==="0xfeaf968c") return Promise.resolve(overrides.round||round());
  }
  throw Error("unexpected RPC call");
};
test("worst case USD uses EIP-1559 max fee and 10% buffer",()=>{
  assert.equal(gasUsdE8({gasLimit:100000n,maxFeePerGasWei:10000000000n,ethUsdE8:300000000000n}),330000000n);
  assert.equal(gasUsdE8({gasLimit:100000n,maxFeePerGasWei:20000000000n,ethUsdE8:300000000000n}),660000000n);
});
test("rejects missing caps and uint256 overflow",()=>{
  assert.throws(()=>gasUsdE8({gasLimit:0n,maxFeePerGasWei:1n,ethUsdE8:1n}));
  assert.throws(()=>gasUsdE8({gasLimit:(1n<<256n)-1n,maxFeePerGasWei:2n,ethUsdE8:1n}));
});
test("real mainnet oracle interface yields both allowed and blocked outcomes",async()=>{
  const a=await inspectGasBudget({rpc:valid(),gasLimit:100000n,maxFeePerGasWei:10000000000n});
  assert.equal(a.allowedByFiveUsd,true);assert.equal(a.gasUsdE8,"330000000");
  const b=await inspectGasBudget({rpc:valid(),gasLimit:100000n,maxFeePerGasWei:20000000000n});
  assert.equal(b.allowedByFiveUsd,false);
});
test("invalid chain, decimals, stale/future round and incomplete round all fail closed",async()=>{
  for(const rpc of [
    valid({chain:"0x2105"}),valid({decimals:"0x12"}),valid({ts:hex(9000n)}),
    valid({round:round(-1n,2n,2n,1000n)}),
    valid({round:round(300000000000n,3n,2n,1000n)}),
    valid({round:round(300000000000n,1n,1n,1200n)})
  ]) await assert.rejects(inspectGasBudget({rpc,gasLimit:100000n,maxFeePerGasWei:10000000000n}));
});
