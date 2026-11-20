#!/usr/bin/env node
// Read-only V2 WETH/USDT and WETH/DAI discovery. NEVER signs or submits.
import { pathToFileURL } from "node:url";
export const ADDRESSES={
 weth:"0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
 usdt:"0xdAC17F958D2ee523a2206206994597C13D831ec7",
 dai:"0x6B175474E89094C44Da98b954EedeAC495271d0F",
 uniFactory:"0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f",
 sushiFactory:"0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac",
 aavePool:"0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2"
};
const SIG={getPair:"0xe6a43905",token0:"0x0dfe1681",token1:"0xd21220a7",
 factory:"0xc45a0155",getReserves:"0x0902f1ac",decimals:"0x313ce567",
 premium:"0x074b2e43"};
const WEI=10n**18n,ZERO="0x"+"0".repeat(40);
const same=(a,b)=>a.toLowerCase()===b.toLowerCase();
const addrArg=a=>a.slice(2).toLowerCase().padStart(64,"0");
const ceil=(a,b)=>(a+b-1n)/b;
const w=(v,i=0)=>{
 if(typeof v!=="string"||!/^0x[\da-fA-F]+$/.test(v)||v.length<2+64*(i+1))
  throw Error("Malformed ABI response");
 return BigInt("0x"+v.slice(2+64*i,2+64*(i+1)));
};
const readAddr=v=>"0x"+w(v).toString(16).padStart(40,"0");
export function v2Quote(input,rin,rout){
 if(input<=0n||rin<=0n||rout<=0n)throw Error("Invalid reserves or input");
 return input*997n*rout/(rin*1000n+input*997n);
}
export function evaluateWethRoute(first,second,amountWei,premiumBps,gasWei){
 if(amountWei<=0n||premiumBps<0n||premiumBps>1000n||gasWei<=0n)
  throw Error("Invalid risk assumptions");
 const intermediateTokenRaw=v2Quote(amountWei,first.wethReserve,first.otherReserve);
 const outputWei=v2Quote(intermediateTokenRaw,second.otherReserve,second.wethReserve);
 const premiumWei=ceil(amountWei*premiumBps,10000n);
 const marginBeforeGasWei=outputWei-amountWei-premiumWei;
 const estimatedPostGasWei=marginBeforeGasWei-gasWei;
 return {amountWei,intermediateTokenRaw,outputWei,premiumWei,marginBeforeGasWei,
  estimatedPostGasWei,theoreticalPositive:estimatedPostGasWei>0n,approvedForExecution:false};
}
export function assessMarket(market,{baseFeeWei,premiumBps,priorityFeeWei=1000000000n,
 gasUnits=400000n,gasBufferBps=2000n,
 amountsWei=[10n**16n,5n*10n**16n,10n**17n,5n*10n**17n,WEI]}={}){
 if(!market||market.missing)return {token:market?.token??"unknown",status:"UNAVAILABLE",
  reason:market?.reason??"missing pair",approvedForExecution:false,routes:[]};
 if(baseFeeWei<0n||premiumBps<0n||priorityFeeWei<0n||gasUnits<=0n||
    gasBufferBps<0n||gasBufferBps>10000n)throw Error("Invalid gas assumptions");
 const gasWei=ceil(gasUnits*(baseFeeWei+priorityFeeWei)*(10000n+gasBufferBps),10000n);
 const routes=[];
 for(const [route,first,second] of [
  ["Uniswap V2 → Sushi V2",market.uni,market.sushi],
  ["Sushi V2 → Uniswap V2",market.sushi,market.uni]
 ])for(const amountWei of amountsWei)
  routes.push({route,...evaluateWethRoute(first,second,amountWei,premiumBps,gasWei)});
 const shallowWethReserve=market.uni.wethReserve<market.sushi.wethReserve?
  market.uni.wethReserve:market.sushi.wethReserve;
 return {token:market.token,status:"SAMPLED_READ_ONLY",uni:market.uni,sushi:market.sushi,
  shallowWethReserve,hypotheticalGasWei:gasWei,gasEstimateVerified:false,
  executableCalldataVerified:false,transactionSubmissionEnabled:false,
  approvedForExecution:false,routes};
}
export async function getSnapshot(rpc,{blockTag="finalized"}={}){
 if(!/^(latest|safe|finalized|0x[\da-fA-F]+)$/.test(blockTag))
  throw Error("Invalid block selector");
 if(BigInt(await rpc("eth_chainId",[]))!==1n)
  throw Error("Wrong chain: requires Ethereum mainnet");
 const block=await rpc("eth_getBlockByNumber",[blockTag,false]);
 if(!block?.number||!block?.timestamp||!block?.baseFeePerGas||!block?.hash)
  throw Error("Missing EIP-1559 block");
 const number=BigInt(block.number);
 if(blockTag.startsWith("0x")&&BigInt(blockTag)!==number)
  throw Error("RPC returned different block");
 const pinned="0x"+number.toString(16);
 const call=(a,d)=>rpc("eth_call",[{to:a,data:d},pinned]);
 async function market(token,other){
  const [dec,uniAddr,sushiAddr,tokenCode]=await Promise.all([
   call(other,SIG.decimals),
   call(ADDRESSES.uniFactory,SIG.getPair+addrArg(ADDRESSES.weth)+addrArg(other)),
   call(ADDRESSES.sushiFactory,SIG.getPair+addrArg(ADDRESSES.weth)+addrArg(other)),
   rpc("eth_getCode",[other,pinned])
  ]);
  const decimals=Number(w(dec));
  if(!tokenCode||tokenCode==="0x"||decimals<1||decimals>18)
   throw Error("Invalid token code or decimals");
  const ua=readAddr(uniAddr),sa=readAddr(sushiAddr);
  if(same(ua,ZERO)||same(sa,ZERO))return {token,other,decimals,missing:true,
   reason:"one or both canonical factories have no pair",uniAddress:ua,sushiAddress:sa};
  async function pair(a,fac){
   const [code,t0,t1,reportedFactory,res]=await Promise.all([
    rpc("eth_getCode",[a,pinned]),call(a,SIG.token0),call(a,SIG.token1),
    call(a,SIG.factory),call(a,SIG.getReserves)
   ]);
   if(!code||code==="0x")throw Error("Pair code missing");
   const token0=readAddr(t0),token1=readAddr(t1);
   if(!same(readAddr(reportedFactory),fac)||
      !((same(token0,ADDRESSES.weth)&&same(token1,other))||
        (same(token1,ADDRESSES.weth)&&same(token0,other))))
    throw Error("Pair/factory identity mismatch");
   const r0=w(res),r1=w(res,1);
   const wethReserve=same(token0,ADDRESSES.weth)?r0:r1;
   const otherReserve=same(token0,ADDRESSES.weth)?r1:r0;
   if(wethReserve<=0n||otherReserve<=0n)throw Error("Empty pool");
   return {address:a,factory:fac,token0,token1,wethReserve,otherReserve};
  }
  const [uni,sushi]=await Promise.all([
   pair(ua,ADDRESSES.uniFactory),pair(sa,ADDRESSES.sushiFactory)
  ]);
  return {token,other,decimals,missing:false,uni,sushi};
 }
 const [wc,wd,ac,premium,usdt,dai]=await Promise.all([
  rpc("eth_getCode",[ADDRESSES.weth,pinned]),
  call(ADDRESSES.weth,SIG.decimals),
  rpc("eth_getCode",[ADDRESSES.aavePool,pinned]),
  call(ADDRESSES.aavePool,SIG.premium),
  market("USDT",ADDRESSES.usdt),market("DAI",ADDRESSES.dai)
 ]);
 if(!wc||wc==="0x"||Number(w(wd))!==18||!ac||ac==="0x")
  throw Error("WETH/Aave code or decimals invalid");
 const premiumBps=w(premium);
 if(premiumBps>1000n)throw Error("Aave premium out of range");
 return {chainId:1,blockNumber:number,blockHash:block.hash,
  blockTime:BigInt(block.timestamp),baseFeeWei:BigInt(block.baseFeePerGas),
  premiumBps,markets:[usdt,dai]};
}
export function assessSnapshot(snapshot,opts={}){
 return {blockNumber:snapshot.blockNumber,blockHash:snapshot.blockHash,
  premiumBps:snapshot.premiumBps,markets:snapshot.markets.map(m=>assessMarket(m,
  {baseFeeWei:snapshot.baseFeeWei,premiumBps:snapshot.premiumBps,...opts})),
  assumptions:{gasUnits:opts.gasUnits??400000n,priorityFeeWei:opts.priorityFeeWei??1000000000n,
   gasBufferBps:opts.gasBufferBps??2000n,gasEstimateVerified:false,
   executableCalldataVerified:false,transactionSubmissionEnabled:false}};
}
export async function jsonRpc(url,method,params=[],request=fetch){
 if(typeof url!=="string"||!/^https:\/\//i.test(url))
  throw Error("HTTPS_URL is required privately");
 let response;
 try{response=await request(url,{method:"POST",headers:{"content-type":"application/json"},
  body:JSON.stringify({jsonrpc:"2.0",id:1,method,params})});}
 catch{throw Error("RPC connection failed (credentials hidden)");}
 if(!response.ok)throw Error("RPC HTTP error "+response.status);
 let body;try{body=await response.json();}catch{throw Error("RPC returned invalid JSON");}
 if(body?.error||body?.result==null)throw Error("RPC rejected "+method);
 return body.result;
}
if(process.argv[1]&&pathToFileURL(process.argv[1]).href===import.meta.url){
 try{
  const snap=await getSnapshot((method,args)=>jsonRpc(process.env.HTTPS_URL,method,args),
   {blockTag:process.env.V2_BLOCK_TAG||"finalized"});
  const result=assessSnapshot(snap);
  console.log(JSON.stringify({snapshot:snap,...result},(_,v)=>typeof v==="bigint"?v.toString():v,2));
 }catch(e){console.error("Read-only research failed:",e.message);process.exitCode=2;}
}
