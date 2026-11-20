// One-candidate, <=6 remote reads. Never calls send/sign, quotes profits, or substitutes block-end state.
import fs from 'node:fs';import crypto from 'node:crypto';
import {selectFirstSwap,gethAfterTransactionPlan} from './first_swap_replay.mjs';
import {verifySelectedReceipt} from './swap_trigger_candidates.mjs';
const endpoint=process.env.HTTPS_URL||'https://ethereum-rpc.publicnode.com';
if(!/^https:\/\//i.test(endpoint))throw Error('HTTPS_RPC_REQUIRED');
const dir=new URL('../evidence/first-swap-replay-20261117/',import.meta.url);
const original=new URL('../evidence/recent-swap-sample.json',import.meta.url),bytes=fs.readFileSync(original),sample=JSON.parse(bytes);
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const preservation=JSON.parse(fs.readFileSync(new URL('preservation.json',dir)));
if(sha(bytes)!==preservation.sampleSha256)throw Error('PRESERVED_SAMPLE_CHANGED');
const selected=selectFirstSwap(sample),requests=[];let count=0;
const evidence={status:'INSUFFICIENT_REPLAY_ACCESS',selected,source:process.env.HTTPS_URL?'configured HTTPS process binding':'temporary public PublicNode HTTPS',sampleSha256Before:sha(bytes),requestBudget:6,requests,postTransactionStateVerified:false,endOfBlockStateUsed:false,opportunityEvaluation:'NOT_RUN_REPLAY_REQUIRED',netProfitVerified:false,tradeExecutionAllowed:false};
const redact=s=>String(s).replace(/https?:\/\/[^\s"']+/gi,'[REDACTED_URL]').slice(0,400);
async function rpc(method,params){
 if(!['eth_chainId','eth_getTransactionReceipt','eth_getBlockByNumber','debug_traceCall','eth_getStorageAt','web3_clientVersion'].includes(method))throw Error('RPC_METHOD_FORBIDDEN');
 if(++count>6)throw Error('RPC_BUDGET_EXCEEDED');
 const record={method,params};requests.push(record);
 let response;try{response=await fetch(endpoint,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:count,method,params}),signal:AbortSignal.timeout(20000)});}catch{record.failure='NETWORK_OR_TIMEOUT';throw Error(record.failure);}
 record.httpStatus=response.status;const body=await response.json().catch(()=>null);
 if(body?.error)record.rpcError={code:body.error.code,message:redact(body.error.message)};
 if(!response.ok||body?.error||body?.result==null)throw Error('RPC_'+method+'_HTTP_'+response.status+(body?.error?'_CODE_'+body.error.code:''));
 return body.result;
}
try{
 if(BigInt(await rpc('eth_chainId',[]))!==1n)throw Error('WRONG_CHAIN');
 const receipt=await rpc('eth_getTransactionReceipt',[selected.txHash]);
 const block=await rpc('eth_getBlockByNumber',['0x'+selected.blockNumber.toString(16),false]);
 verifySelectedReceipt(selected,receipt,block);
 if(BigInt(block.baseFeePerGas)!==BigInt(selected.historicalBaseFeeWei))throw Error('BASE_FEE_CHANGED');
 fs.writeFileSync(new URL('selected-receipt.json',dir),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
 fs.writeFileSync(new URL('selected-block.json',dir),JSON.stringify(block,null,2)+'\n',{flag:'wx'});
 evidence.receiptAndCanonicalPositionReverified=true;evidence.historicalBaseFeeWei=BigInt(block.baseFeePerGas).toString();
 const plan=gethAfterTransactionPlan(selected,block);evidence.plan=plan;
 try{const trace=await rpc(plan.method,plan.params);evidence.trace=trace;evidence.status='TRACE_RETURNED_BOUNDARY_NOT_YET_CERTIFIED';}
 catch(e){evidence.traceFailure=redact(e.message);
  // At most one parent-state prerequisite test, never an automatic multi-transaction replay/download.
  try{const state=await rpc('eth_getStorageAt',['0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2','0x0','0x'+(selected.blockNumber-1).toString(16)]);evidence.anvilParentStatePrerequisite={status:'AVAILABLE_FOR_THIS_SLOT_ONLY',raw:state};}
  catch(e2){evidence.anvilParentStatePrerequisite={status:'BLOCKED',error:redact(e2.message)};}
  evidence.anvilStarted=false;
 }
}catch(e){evidence.failure=redact(e.message);}
finally{evidence.rpcCalls=count;evidence.sampleSha256After=sha(fs.readFileSync(original));evidence.originalSampleUnchanged=evidence.sampleSha256Before===evidence.sampleSha256After;
 fs.writeFileSync(new URL('replay-capability.json',dir),JSON.stringify(evidence,null,2)+'\n',{flag:'wx'});
 console.log(JSON.stringify({status:evidence.status,rpcCalls:count,originalSampleUnchanged:evidence.originalSampleUnchanged,receiptReverified:evidence.receiptAndCanonicalPositionReverified,traceFailure:evidence.traceFailure,anvilPrerequisite:evidence.anvilParentStatePrerequisite?.status}));}
