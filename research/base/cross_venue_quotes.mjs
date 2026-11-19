#!/usr/bin/env node
// Base 8453: bounded, same-block, two-venue quote screening. READ-ONLY.
// Expected V3 QuoterV2 ABI: quoteExactInputSingle((address,address,uint256,uint24,uint160)).
// This is NOT an atomic execution simulation, gas estimation, or trade approval.
import { pathToFileURL } from "node:url";

export const TOKENS = Object.freeze({
  WETH:"0x4200000000000000000000000000000000000006",
  USDC:"0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"
});
export const VENUES = Object.freeze({
  uniswap:{factory:"0x33128a8fc17869897dce68ed026d694621f6fdd",
    quoter:"0x3d4e44eb1374240ce5f1b871ab261cd16335b76a",
    pool:"0xd0b53d9277642d899df5c87a3966a349a798f224",fee:500n},
  pancake:{factory:"0x0bfbcf9fa4f9c56b0f40a671ad40e0805a091865",
    quoter:"0xb048bbc1ee6b733fffcfb9e9cef7375518e25997",
    pool:"0x72ab388e2e2f6facef59e3c3fa2c4e29011c2d38",fee:100n}
});
const word=x=>{const n=BigInt(x);if(n<0n||n>=2n**256n)throw Error("Invalid U256");return n.toString(16).padStart(64,"0");};
const address=x=>{if(!/^0x[0-9a-f]{40}$/i.test(x))throw Error("Invalid address");return x.slice(2).toLowerCase().padStart(64,"0");};
const values=(data,n)=>{
 if(typeof data!=="string"||!/^0x[0-9a-f]+$/i.test(data)||data.length!==2+64*n)throw Error("Invalid ABI result");
 return Array.from({length:n},(_,i)=>BigInt("0x"+data.slice(2+64*i,2+64*(i+1))));
};
export const factoryData=v=>"0x1698ee82"+address(TOKENS.WETH)+address(TOKENS.USDC)+word(v.fee);
export const quoteData=(v,tokenIn,tokenOut,amount)=>
 "0xc6a5026a"+address(tokenIn)+address(tokenOut)+word(amount)+word(v.fee)+word(0);
export async function compare(fetcher=fetch,endpoint=process.env.BASE_RPC_URL||"https://mainnet.base.org"){
 const url=new URL(endpoint);
 if(url.protocol!=="https:"||url.username||url.password||url.search||url.hash)throw Error("Use HTTPS RPC without URL query credentials");
 let requests=0;
 async function rpc(method,params,stage="rpc"){
  if(++requests>18)throw Error("RPC budget exhausted");
  const res=await fetcher(endpoint,{method:"POST",
    headers:{"content-type":"application/json"},
    body:JSON.stringify({jsonrpc:"2.0",id:requests,method,params}),
    signal:AbortSignal.timeout(8000)});
  if(!res.ok)throw Error(stage+" HTTP "+res.status);
  const payload=await res.json();
  if(payload.error||typeof payload.result!=="string")throw Error(stage+" RPC refused"+(Number.isInteger(payload.error?.code)?" code="+payload.error.code:""));
  return payload.result;
 }
 if((await rpc("eth_chainId",[],"chainId")).toLowerCase()!=="0x2105")throw Error("Wrong chain");
 const block=await rpc("eth_blockNumber",[],"blockNumber");
 if(!/^0x[0-9a-f]+$/i.test(block))throw Error("Invalid block");
 for(const [label,v] of Object.entries(VENUES)){
  const returned=values(await rpc("eth_call",[{to:v.factory,data:factoryData(v)},block],label+" factory.getPool"),1)[0];
  if(returned!==BigInt(v.pool))throw Error(label+" canonical factory pool mismatch");
 }
 async function quote(label,tokenIn,tokenOut,amountIn){
  const v=VENUES[label];
  const result=values(await rpc("eth_call",[{to:v.quoter,
    data:quoteData(v,tokenIn,tokenOut,amountIn)},block],label+" quoter.quoteExactInputSingle"),4);
  if(result[0]===0n)throw Error(label+" zero amount out");
  return result[0];
 }
 const results=[];
 const amounts=[100n,500n,1000n].map(v=>v*1000000n);
 for(const first of ["uniswap","pancake"]){
  const second=first==="uniswap"?"pancake":"uniswap";
  for(const amountIn of amounts){
   const wethOut=await quote(first,TOKENS.USDC,TOKENS.WETH,amountIn);
   const usdcOut=await quote(second,TOKENS.WETH,TOKENS.USDC,wethOut);
   const delta=usdcOut-amountIn;
   results.push({first,second,block,
      amountInUSDC:(Number(amountIn)/1e6).toFixed(2),
      amountOutUSDC:(Number(usdcOut)/1e6).toFixed(6),
      pnlBeforeGasUSDC:(Number(delta)/1e6).toFixed(6),
      grossPositiveBeforeGas:delta>0n});
  }
 }
 return {chainId:8453,block,rpcCalls:requests,canonicalFactoryPoolsVerified:true,
   results,gasVerified:false,l1DataFeeVerified:false,capitalFundingVerified:false,
   executableTransactionVerified:false,profitVerified:false,txSubmitted:0,
   interpretation:"Same-block spot quotes only; not realizable post-gas arbitrage."};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href)
 compare().then(r=>console.log(JSON.stringify(r,null,2))).catch(e=>{
  console.error(JSON.stringify({status:"UNVERIFIED",reason:e.message,txSubmitted:0}));
  process.exitCode=1;
 });
