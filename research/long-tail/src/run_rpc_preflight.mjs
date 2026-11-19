// One <=6-request log diagnostic plus a separate <=2-request state diagnostic.
// No retry or backfill. Never retain RPC URLs or arbitrary provider error text.
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {Rpc} from './chain.mjs';
import {diagnoseRpc} from './rpc_capability.mjs';
import {diagnoseState} from './rpc_state_capability.mjs';
const checkpointPath=process.env.DISCOVERY_CHECKPOINT;
if(!checkpointPath)throw Error('DISCOVERY_CHECKPOINT required');
const data=await readFile(checkpointPath),checkpoint=JSON.parse(data);
const hash=x=>createHash('sha256').update(x).digest('hex');
const requests=[],native=globalThis.fetch;
globalThis.fetch=async(url,options)=>{
 const q=JSON.parse(options.body);const entry={method:q.method,params:q.params};requests.push(entry);
 let response;try{response=await native(url,options);}catch{entry.transport='NETWORK_OR_TIMEOUT';throw Error('RPC_NETWORK_OR_TIMEOUT');}
 entry.httpStatus=response.status;
 entry.quotaHeaders={};for(const name of ['retry-after','ratelimit-limit','ratelimit-remaining','ratelimit-reset','x-ratelimit-limit','x-ratelimit-remaining','x-ratelimit-reset']){const v=response.headers.get(name);if(v!==null)entry.quotaHeaders[name]=v;}
 try{const body=await response.clone().json();if(body?.error){entry.providerErrorCode=body.error.code;const message=String(body.error.message||'');entry.providerErrorCategory=/archive requests require a personal token/i.test(message)?'ARCHIVE_PERSONAL_TOKEN_REQUIRED':/rate|quota|limit exceeded|too many/i.test(message)?'LIMIT_OR_QUOTA_MESSAGE':'OTHER_ERROR_TEXT_NOT_RETAINED';}}
 catch{entry.bodyInspection='NOT_JSON';}
 return response;
};
const outcome={source:process.env.HTTPS_URL?'configured HTTPS process binding':'temporary public PublicNode HTTPS',checkpointSha256Before:hash(data),requestCaps:{logDiagnostic:6,separateStateDiagnostic:2,total:8},requests};
try{
 const url=process.env.HTTPS_URL||'https://ethereum-rpc.publicnode.com';
 const rpc=new Rpc(url,{maxCalls:6});outcome.logs=await diagnoseRpc(rpc,checkpoint);outcome.logRequestCount=rpc.count;
 const stateRpc=new Rpc(url,{maxCalls:2});outcome.state=await diagnoseState(stateRpc,outcome.logs.olderBlock);outcome.stateRequestCount=stateRpc.count;
} catch(e){outcome.error=String(e.message).slice(0,130);process.exitCode=2;}
finally{globalThis.fetch=native;outcome.checkpointSha256After=hash(await readFile(checkpointPath));outcome.checkpointUnchanged=outcome.checkpointSha256Before===outcome.checkpointSha256After;
 const output=process.env.DIAG_OUTPUT;if(!output)throw Error('DIAG_OUTPUT required');await writeFile(output,JSON.stringify(outcome,null,2)+'\n');console.log(JSON.stringify({logRequests:outcome.logRequestCount,stateRequests:outcome.stateRequestCount,totalRequests:requests.length,checkpointUnchanged:outcome.checkpointUnchanged,finding:outcome.logs?.finding,error:outcome.error}));}
