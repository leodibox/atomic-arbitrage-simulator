#!/usr/bin/env node
// Bounded, read-only Base recent Swap-event sampler. NO historical profit or execution claims.
// Different topic0 for Uniswap V3 and PancakeSwap V3 is intentional.
import {pathToFileURL} from "node:url";
export const POOLS = Object.freeze([
 {name:"uniswap-v3-weth-usdc-500",
  address:"0xd0b53d9277642d899df5c87a3966a349a798f224",
  topic:"0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67"},
 {name:"pancakeswap-v3-weth-usdc-100",
  address:"0x72ab388e2e2f6facef59e3c3fa2c4e29011c2d38",
  topic:"0x19b47279256b2a23a1665c810c8d55a1758940ee09377d4f8d26497a3577dc83"}
]);
const isHex=(s,n)=>typeof s==="string"&&new RegExp("^0x[0-9a-fA-F]{"+n+"}$").test(s);
export function checkLog(x, pool, lo, hi){
 if(!x||x.removed===true||x.address?.toLowerCase()!==pool.address||
    x.topics?.[0]?.toLowerCase()!==pool.topic)return false;
 if(!isHex(x.transactionHash,64)||!isHex(x.blockHash,64))return false;
 if(typeof x.blockNumber!=="string"||!/^0x[0-9a-f]+$/i.test(x.blockNumber))return false;
 const b=BigInt(x.blockNumber);if(b<lo||b>hi)return false;
 if(!/^0x[0-9a-f]+$/i.test(x.logIndex||""))return false;
 return true;
}
export async function sample(fetcher=fetch,endpoint=process.env.BASE_RPC_URL||"https://base-rpc.publicnode.com"){
 const u=new URL(endpoint);
 if(u.protocol!=="https:"||u.username||u.password||u.search||u.hash)throw Error("HTTPS RPC URL without embedded credentials required");
 let calls=0;
 async function rpc(method,params,step){
  if(++calls>14)throw Error("Read budget exhausted");
  const resp=await fetcher(endpoint,{method:"POST",headers:{"content-type":"application/json"},
   body:JSON.stringify({jsonrpc:"2.0",id:calls,method,params}),signal:AbortSignal.timeout(9000)});
  if(!resp.ok)throw Error(step+" HTTP "+resp.status);
  const out=await resp.json();
  if(out?.error||out?.result==null)throw Error(step+" RPC refused"+(Number.isInteger(out?.error?.code)?" code="+out.error.code:""));
  return out.result;
 }
 if((await rpc("eth_chainId",[],"chain")).toLowerCase()!=="0x2105")throw Error("Wrong chain");
 const headHex=await rpc("eth_blockNumber",[],"head");
 if(!/^0x[0-9a-f]+$/i.test(headHex))throw Error("Invalid head");
 const head=BigInt(headHex);
 const maxEvents=2000, span=100n, windows=6;
 let filtered=0, scanned=0;
 const hashes=new Set(), events=[];
 let partial=false;
 // Last 600 blocks, two distinct pools, at most 12 getLogs reads;
 // fail closed on errors, do not silently count incomplete history as coverage.
 for(let w=0;w<windows;w++){
  const hi=head-BigInt(w)*span;
  const lo=hi>=span-1n?hi-(span-1n):0n;
  for(const pool of POOLS){
   const data=await rpc("eth_getLogs",[{fromBlock:"0x"+lo.toString(16),toBlock:"0x"+hi.toString(16),
    address:pool.address,topics:[pool.topic]}],pool.name+" getLogs");
   if(!Array.isArray(data))throw Error("getLogs not an array");
   scanned+=data.length;
   for(const x of data){
    if(!checkLog(x,pool,lo,hi)){filtered++;continue;}
    const key=pool.name+":"+x.transactionHash.toLowerCase()+":"+x.logIndex.toLowerCase();
    if(hashes.has(key))continue;
    hashes.add(key);
    if(events.length<maxEvents)events.push({pool:pool.name,block:x.blockNumber,
      txHash:x.transactionHash,logIndex:x.logIndex});
   }
  }
  if(events.length===maxEvents){partial=true;break;}
 }
 const blocks=[...new Set(events.map(x=>x.block))];
 return {chainId:8453,head:headHex,requestedWindows:windows,windowWidthBlocks:Number(span),
  calls,rawLogs:scanned,rejectedLogs:filtered,validEvents:events.length,uniqueBlocks:blocks.length,
  truncatedAtEventLimit:partial,historyCoverageComplete:!partial,
  eligibleForProfitClaims:false,preGasPositiveCount:null,netPositiveCount:null,
  historicalStateVerified:false,transactionsSent:0,events};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 sample().then(s=>console.log(JSON.stringify(s,null,2))).catch(e=>{
  console.error(JSON.stringify({status:"UNVERIFIED",reason:e.message,transactionsSent:0}));
  process.exitCode=1;
 });
}
