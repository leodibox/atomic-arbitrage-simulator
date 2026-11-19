#!/usr/bin/env node
// Bounded, read-only exact single-pool Uniswap V3 quotes on Base. NOT an arbitrage signal.
import { pathToFileURL } from "node:url";
export const CHAIN_ID="0x2105";
export const FACTORY="0x33128a8fc17869897dce68ed026d694621f6fdfd";
export const QUOTER_V2="0x3d4e44eb1374240ce5f1b871ab261cd16335b76a";
export const POOL="0xd0b53d9277642d899df5c87a3966a349a798f224";
export const WETH="0x4200000000000000000000000000000000000006";
export const USDC="0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";
export const FEE=500n;
export const encodeAddress=a=>{
 if(!/^0x[0-9a-fA-F]{40}$/.test(a)) throw new Error("Invalid address");
 return a.slice(2).toLowerCase().padStart(64,"0");
};
export const encodeUint=u=>{
 u=BigInt(u);if(u<0n||u>=2n**256n) throw new Error("Invalid uint");
 return u.toString(16).padStart(64,"0");
};
export const decodeWords=(raw,n)=>{
 if(typeof raw!=="string"||!/^0x[0-9a-f]*$/i.test(raw)||raw.length!==2+64*n) throw new Error("Malformed ABI response");
 return Array.from({length:n},(_,i)=>BigInt("0x"+raw.slice(2+i*64,2+(i+1)*64)));
};
export const factoryCallData=()=>"0x1698ee82"+encodeAddress(WETH)+encodeAddress(USDC)+encodeUint(FEE);
export const quoterCallData=(tokenIn,tokenOut,amountIn)=>"0xc6a5026a"+encodeAddress(tokenIn)+encodeAddress(tokenOut)+encodeUint(amountIn)+encodeUint(FEE)+encodeUint(0);
export async function queryUniswap(fetcher=fetch,endpoint=process.env.BASE_RPC_URL||"https://mainnet.base.org"){
 for(const [label,v] of Object.entries({FACTORY,QUOTER_V2,POOL,WETH,USDC})){
  if(!/^0x[0-9a-f]{40}$/i.test(v))throw Error("Malformed "+label+" address");
 }
 const u=new URL(endpoint);
 if(u.protocol!=="https:"||u.username||u.password||u.search||u.hash) throw new Error("HTTPS URL without query credentials required");
 let calls=0;
 async function rpc(method,params){
  if(++calls>12) throw new Error("RPC budget exceeded");
  const response=await fetcher(endpoint,{method:"POST",headers:{"content-type":"application/json"},
   body:JSON.stringify({jsonrpc:"2.0",id:calls,method,params}),signal:AbortSignal.timeout(8000)});
  if(!response.ok) throw new Error(method+" HTTP "+response.status);
  const value=await response.json();
  if(value.error||typeof value.result!=="string") throw new Error(method+" RPC error");
  return value.result;
 }
 if((await rpc("eth_chainId",[])).toLowerCase()!==CHAIN_ID) throw new Error("Wrong chain ID");
 const block=await rpc("eth_blockNumber",[]);
 if(!/^0x[0-9a-f]+$/i.test(block)) throw new Error("Invalid block tag");
 const deployed=decodeWords(await rpc("eth_call",[{to:FACTORY,data:factoryCallData()},block]),1)[0];
 if(deployed!==BigInt(POOL)) throw new Error("Canonical Uniswap factory did not confirm pool");
 const usdcSizes=[100n,500n,1000n,2000n].map(v=>v*1000000n);
 const wethSizes=[5n,10n].map(v=>v*10n**16n);
 const quotes=[];
 for(const [tokenIn,tokenOut,sizes,asset] of [[USDC,WETH,usdcSizes,"USDC"],[WETH,USDC,wethSizes,"WETH"]]){
  for(const amountIn of sizes){
   const data=quoterCallData(tokenIn,tokenOut,amountIn);
   const words=decodeWords(await rpc("eth_call",[{to:QUOTER_V2,data},block]),4);
   if(words[0]===0n) throw new Error("Zero Quoter output");
   quotes.push({asset,amountInRaw:String(amountIn),amountOutRaw:String(words[0]),
    ticksCrossed:Number(words[2]),quoterGasEstimate:String(words[3])});
  }
 }
 return {chainId:8453,block,canonicalUniswapPoolVerified:true,quotes,
  crossVenueArbitrageVerified:false,netProfitVerified:false,gasCostVerified:false,
  txSent:0,rpcCalls:calls};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 queryUniswap().then(v=>console.log(JSON.stringify(v,null,2))).catch(e=>{
  console.error(JSON.stringify({status:"UNVERIFIED",reason:e.message,txSent:0}));process.exitCode=1;
 });
}