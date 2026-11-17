#!/usr/bin/env node
// Bounded, read-only diagnostic of missing historical PairCreated/PoolCreated ranges.
// This DOES NOT fill coverage, change the original checkpoint, or claim profitability.
import {readFile, writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {Rpc,FACTORIES,TOPICS,tag,same} from './chain.mjs';

const accessDenied = reason => /RPC_HTTP_(401|403)/.test(reason);
const resultOf = async task => {
  try {const logs=await task();return {status:'SUCCESS',count:logs.length};}
  catch(e){return {status:'FAILED',error:String(e.message||e).slice(0,100)};}
};

export function locateGap(discovery, factoryName) {
  const group=discovery.find(v=>v.factory?.name===factoryName);
  if(!group||!Array.isArray(group.coverage))throw Error('Missing factory coverage in checkpoint');
  const coverage=[...group.coverage].sort((a,b)=>a.from-b.from);
  for(let i=0;i<coverage.length;i++){
    const v=coverage[i];
    if(!Number.isSafeInteger(v.from)||!Number.isSafeInteger(v.to)||v.from>v.to||
       !['COMPLETE','FAILED'].includes(v.status))throw Error('Invalid coverage entry');
    if(i && v.from!==coverage[i-1].to+1)throw Error('Coverage has gaps or overlaps');
  }
  const missing=coverage.filter(c=>c.status==='FAILED');
  const complete=coverage.filter(c=>c.status==='COMPLETE');
  const failedBlocks=missing.reduce((s,c)=>s+c.to-c.from+1,0);
  const completedBlocks=complete.reduce((s,c)=>s+c.to-c.from+1,0);
  const boundary=missing.map(f=>({failed:f,accepted:complete.find(c=>c.from===f.to+1)}))
    .filter(x=>x.accepted).at(-1);
  if(!boundary)throw Error('No denied-to-successful coverage boundary');
  return {factory:group.factory,failedBlocks,completedBlocks,
    totalBlocks:failedBlocks+completedBlocks,missingWindows:missing.length,
    olderTarget:boundary.failed.from,
    olderWindowEnd:boundary.failed.to,
    olderWindowStart:boundary.failed.from,
    acceptedTarget:boundary.accepted.from};
}

export async function diagnoseRpc(rpc, checkpoint, {factoryName='UniswapV2'}={}) {
  if(checkpoint?.chainId!==1||!checkpoint?.endBlock?.hash||
     !Number.isSafeInteger(checkpoint.endBlock.number))throw Error('Invalid checkpoint chain/hash');
  const gap=locateGap(checkpoint.discovery,factoryName);
  const f=FACTORIES.find(x=>x.name===factoryName);
  if(!f||!same(f.address,gap.factory.address)||f.kind!==gap.factory.kind)
    throw Error('Factory mismatch in checkpoint');
  if(BigInt(await rpc.call('eth_chainId',[]))!==1n)throw Error('Wrong network');
  const header=await rpc.call('eth_getBlockByNumber',[tag(checkpoint.endBlock.number),false]);
  if(!header||header.hash?.toLowerCase()!==checkpoint.endBlock.hash.toLowerCase())
    throw Error('Checkpoint block hash mismatch or unavailable');
  const getLogs=(from,to)=>rpc.call('eth_getLogs',[{
    address:f.address,topics:[TOPICS[f.kind]],fromBlock:tag(from),toBlock:tag(to)
  }]);
  const recent=await resultOf(()=>getLogs(gap.acceptedTarget,gap.acceptedTarget));
  const older=await resultOf(()=>getLogs(gap.olderTarget,gap.olderTarget));
  let olderTen={status:'NOT_TESTED'};
  let olderWindow={status:'NOT_TESTED'};
  let finding, recommended;
  if(recent.status!=='SUCCESS'){
    finding='INCONCLUSIVE_RECENT_PROBE_FAILED';
    recommended='Verify provider configuration and consistent block access; do not backfill';
  }else if(older.status!=='SUCCESS'){
    finding=accessDenied(older.error)?'SINGLE_OLD_BLOCK_ACCESS_DENIED':'SINGLE_OLD_BLOCK_ERROR';
    recommended='Smaller chunks alone cannot fix this observed one-block failure; verify historical-access permissions with provider';
  }else {
    olderTen=await resultOf(()=>getLogs(gap.olderTarget,Math.min(gap.olderWindowEnd,gap.olderTarget+9)));
    if(olderTen.status!=='SUCCESS'){
      finding='RANGE_OR_PROVIDER_LIMIT_AT_TEN_BLOCKS';
      recommended='Confirm permitted range and error family before considering bounded pagination';
    }else {
      olderWindow=await resultOf(()=>getLogs(gap.olderWindowStart,gap.olderWindowEnd));
      finding=olderWindow.status==='SUCCESS'?'OLD_WINDOW_NOW_AVAILABLE':'RANGE_OR_PROVIDER_LIMIT_AT_TWO_THOUSAND_BLOCKS';
      recommended=olderWindow.status==='SUCCESS'
        ?'Original failure might have been transient; backfill with checkpoint/hash validation'
        :'Use provider-supported chunk sizes with bounded requests and an explicit budget';
    }
  }
  return {
    type:'ISSUE15_RPC_READONLY_CAPABILITY',
    chainId:1,blockNumber:checkpoint.endBlock.number,blockHash:checkpoint.endBlock.hash,
    factory:factoryName,olderBlock:gap.olderTarget,recentBlock:gap.acceptedTarget,
    gap:{failedBlocks:gap.failedBlocks,completedBlocks:gap.completedBlocks,
      totalBlocks:gap.totalBlocks,failedWindows:gap.missingWindows},
    probes:{recentOneBlock:recent,oldOneBlock:older,oldTenBlocks:olderTen,oldOriginalWindow:olderWindow},
    finding,recommended,originalCoverageUpdated:false,
    approvedForExecution:false,transactionSubmissionEnabled:false
  };
}

if(process.argv[1] && path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
  try {
    const checkpointPath=process.env.DISCOVERY_CHECKPOINT;
    if(!checkpointPath)throw Error('DISCOVERY_CHECKPOINT path is required');
    const checkpoint=JSON.parse(await readFile(checkpointPath,'utf8'));
    // Six RPC calls at most: chain ID, header, two 1-block, 10-block, and original window.
    const rpc=new Rpc(process.env.HTTPS_URL,{maxCalls:6});
    const outcome=await diagnoseRpc(rpc,checkpoint,{
      factoryName:process.env.DIAG_FACTORY||'UniswapV2'
    });
    const rendered=JSON.stringify(outcome,null,2)+'\n';
    const outputFile=process.env.DIAG_OUTPUT;
    if(outputFile){await writeFile(outputFile,rendered);console.log('Result written; original checkpoint unchanged');}
    else console.log(rendered);
  }catch(e){console.error('RPC capability diagnostic failed closed:',String(e.message||e).slice(0,130));process.exitCode=2;}
}
