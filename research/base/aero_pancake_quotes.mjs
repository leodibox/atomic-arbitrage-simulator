#!/usr/bin/env node
// Base 8453: read-only same-block comparison of Pancake V3 and Aerodrome Slipstream V1.
// NO signing, submission, private keys, paid infrastructure or live profit claim.
// Aerodrome uses int24 tickSpacing, NOT the Uniswap/Pancake fee-tier ABI.
import {pathToFileURL} from "node:url";
export const WETH="0x4200000000000000000000000000000000000006";
export const USDC="0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";
export const PANCAKE=Object.freeze({
 pool:"0x72ab388e2e2f6facef59e3c3fa2c4e29011c2d38",
 factory:"0x0bfbcf9fa4f9c56b0f40a671ad40e0805a091865",
 quoter:"0xb048bbc1ee6b733fffcfb9e9cef7375518e25997",fee:100n,
});
export const AERO=Object.freeze({
 pool:"0xdbc6998296caa1652a810dc8d3baf4a8294330f1",
 factory:"0x5e7bb104d84c7cb9b682aac2f3d509f5f406809a",
 quoter:"0x254cf9e1e6e233aa1ac962cb9b05b2cfeaae15b0",
});
const word=n=>{n=BigInt(n);if(n<0n||n>=2n**256n)throw Error("Invalid UINT256");return n.toString(16).padStart(64,"0");};
const addr=a=>{if(!/^0x[0-9a-f]{40}$/i.test(a))throw Error("Invalid address");return a.slice(2).toLowerCase().padStart(64,"0");};
const decode=(s,n=1)=>{if(typeof s!=="string"||!/^0x[0-9a-f]+$/i.test(s)||s.length!==2+n*64)throw Error("Malformed ABI response");return Array.from({length:n},(_,i)=>BigInt("0x"+s.slice(2+64*i,2+64*(i+1))));};
export const pancakeFactoryData=()=>"0x1698ee82"+addr(WETH)+addr(USDC)+word(PANCAKE.fee);
export const pancakeQuoteData=(tokenIn,tokenOut,amount)=>
 "0xc6a5026a"+addr(tokenIn)+addr(tokenOut)+word(amount)+word(PANCAKE.fee)+word(0);
export const aeroQuoteData=(tokenIn,tokenOut,amount,tickSpacing)=>
 "0x9e7defe6"+addr(tokenIn)+addr(tokenOut)+word(amount)+word(tickSpacing)+word(0);
export async function inspect(fetcher=fetch, endpoint=process.env.BASE_RPC_URL||"https://base-rpc.publicnode.com"){
 for(const a of [WETH,USDC,...Object.values(PANCAKE).filter(x=>typeof x==="string"),...Object.values(AERO)]){
  if(typeof a==="string"&&!/^0x[0-9a-f]{40}$/i.test(a))throw Error("Malformed configured address");
 }
 const url=new URL(endpoint);
 if(url.protocol!=="https:"||url.username||url.password||url.search||url.hash)throw Error("Use HTTPS RPC without embedded URL credentials");
 let count=0;
 async function rpc(method,params,stage){
  if(++count>26)throw Error("RPC call budget exceeded");
  const res=await fetcher(endpoint,{method:"POST",headers:{"content-type":"application/json"},
   body:JSON.stringify({jsonrpc:"2.0",id:count,method,params}),signal:AbortSignal.timeout(9000)});
  if(!res.ok)throw Error(stage+" HTTP "+res.status);
  const reply=await res.json();
  if(reply?.error||typeof reply?.result!=="string")throw Error(stage+" refused"+(Number.isInteger(reply.error?.code)?" code="+reply.error.code:""));
  return reply.result;
 }
 if((await rpc("eth_chainId",[],"chain")).toLowerCase()!=="0x2105")throw Error("Not Base mainnet");
 const block=await rpc("eth_blockNumber",[],"block");
 if(!/^0x[0-9a-f]+$/i.test(block))throw Error("Invalid block");
 const call=async(to,data,stage)=>rpc("eth_call",[{to,data},block],stage);
 const pool=decode(await call(PANCAKE.factory,pancakeFactoryData(),"pancake factory.getPool"))[0];
 if(pool!==BigInt(PANCAKE.pool))throw Error("Pancake factory pool mismatch");
 const aeroFactory=decode(await call(AERO.pool,"0xc45a0155","aero pool.factory"))[0];
 if(aeroFactory!==BigInt(AERO.factory))throw Error("Unexpected Aerodrome factory");
 const aeroToken0=decode(await call(AERO.pool,"0x0dfe1681","aero token0"))[0];
 const aeroToken1=decode(await call(AERO.pool,"0xd21220a7","aero token1"))[0];
 if([aeroToken0,aeroToken1].sort((a,b)=>a<b?-1:1).join(",")!==[BigInt(WETH),BigInt(USDC)].sort((a,b)=>a<b?-1:1).join(","))
  throw Error("Aerodrome pool token mismatch");
 const spacing=decode(await call(AERO.pool,"0xd0c93a7c","aero tickSpacing"))[0];
 if(spacing<=0n||spacing>10000n)throw Error("Unsupported Aerodrome tick spacing");
 const liquidity=decode(await call(AERO.pool,"0x1a686502","aero liquidity"))[0];
 if(liquidity===0n)throw Error("No active Aerodrome liquidity");
 // Verify the effective fee: Slipstream fee can be configured by the factory.
 // getSwapFee(address) may be inaccessible on older variants; fail closed.
 // Verified selector from interface required before net-gas decisions.
 const quote=async(venue,tokenIn,tokenOut,amount)=>{
  const isP=venue==="pancake";
  const raw=await call(isP?PANCAKE.quoter:AERO.quoter,
   isP?pancakeQuoteData(tokenIn,tokenOut,amount):aeroQuoteData(tokenIn,tokenOut,amount,spacing),
   venue+" quoter");
  const words=decode(raw,4);
  if(words[0]===0n)throw Error(venue+" zero output");
  return words[0];
 };
 const results=[];
 for(const first of ["pancake","aero"]){
  const second=first==="pancake"?"aero":"pancake";
  for(const amount of [100n,500n,1000n].map(v=>v*1000000n)){
   const weth=await quote(first,USDC,WETH,amount);
   const usdc=await quote(second,WETH,USDC,weth);
   const delta=usdc-amount;
   results.push({first,second,amountInUSDC:(Number(amount)/1e6).toFixed(2),
    amountOutUSDC:(Number(usdc)/1e6).toFixed(6),
    beforeGasUSDC:(Number(delta)/1e6).toFixed(6),
    positiveBeforeGas:delta>0n});
  }
 }
 return {block,chainId:8453,aeroTickSpacing:Number(spacing),aeroLiquidityRaw:liquidity.toString(),
  factoryAndTokensVerified:true,feeVerified:false,gasVerified:false,
  atomicExecutabilityVerified:false,netProfitVerified:false,txSubmitted:0,rpcCalls:count,results};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href)
 inspect().then(v=>console.log(JSON.stringify(v,null,2))).catch(e=>{
  console.error(JSON.stringify({status:"UNVERIFIED",reason:e.message,txSubmitted:0}));process.exitCode=1;
 });
