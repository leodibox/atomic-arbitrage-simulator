#!/usr/bin/env node
// Read-only bounded RPC diagnosis for -32602 at Uniswap Base factory.getPool.
import { VENUES, factoryData } from "./cross_venue_quotes.mjs";
const endpoint=process.env.BASE_RPC_URL||"https://mainnet.base.org";
const u=new URL(endpoint);
if(u.protocol!=="https:"||u.username||u.password||u.search||u.hash)throw Error("Use HTTPS endpoint without credentials in URL");
let count=0;
async function rpc(method,params,label){
 if(++count>8)throw Error("RPC budget exhausted");
 try{
  const resp=await fetch(endpoint,{method:"POST",headers:{"content-type":"application/json"},
    body:JSON.stringify({jsonrpc:"2.0",id:count,method,params}),signal:AbortSignal.timeout(8000)});
  const body=await resp.json();
  const code=Number.isInteger(body?.error?.code)?body.error.code:null;
  const sanitizedMessage=String(body?.error?.message||"").slice(0,160)
    .replace(/https?:\/\/[^\s]+/gi,"[URL]").replace(/0x[0-9a-f]{40,}/gi,"[HEX]");
  const result={step:label,http:resp.status,code,message:sanitizedMessage,
    success:resp.ok&&typeof body.result==="string"};
  if(result.success){
    if(method==="eth_chainId"||method==="eth_blockNumber")result.value=body.result;
    else if(method==="eth_call")result.returnLength=body.result.length;
    else if(method==="eth_getCode")result.bytecodePresent=body.result!=="0x";
  }
  console.log(JSON.stringify(result));
  return result.success?body.result:null;
 }catch(e){console.log(JSON.stringify({step:label,success:false,errorType:e.name}));return null;}
}
const chain=await rpc("eth_chainId",[],"chain");
if(chain!=="0x2105"){console.log("STOP: wrong chain or inaccessible RPC");process.exit(1);}
const block=await rpc("eth_blockNumber",[],"head");
if(!block){process.exit(1);}
const factory=VENUES.uniswap.factory;
await rpc("eth_getCode",[factory,"latest"],"factory bytecode latest");
await rpc("eth_call",[{to:factory,data:factoryData(VENUES.uniswap)},"latest"],"factory getPool latest + data");
await rpc("eth_call",[{to:factory,data:factoryData(VENUES.uniswap)},block],"factory getPool pinned + data");
await rpc("eth_call",[{to:factory,input:factoryData(VENUES.uniswap)},"latest"],"factory getPool latest + input");
console.log(JSON.stringify({readOnly:true,transactionsSent:0,calls:count}));
