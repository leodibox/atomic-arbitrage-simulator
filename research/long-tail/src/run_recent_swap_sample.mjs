#!/usr/bin/env node
// Bounded recent finalized Swap event sampler. Discovery only: NO replay, no sends.
// From research/long-tail: node src/run_recent_swap_sample.mjs
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {id} from 'ethers';
import {buildPoolIndex,TOPICS,rankLargestSwaps,verifySelectedReceipt} from './swap_trigger_candidates.mjs';

function safeInteger(value,def,min,max) {
 const x=value===undefined?def:Number(value);
 if(!Number.isSafeInteger(x)||x<min||x>max)throw Error('INVALID_RESOURCE_LIMIT');
 return x;
}
async function main(){
 if(id('Swap(address,uint256,uint256,uint256,uint256,address)').toLowerCase()!==TOPICS.v2 ||
    id('Swap(address,address,int256,int256,uint160,uint128,int24)').toLowerCase()!==TOPICS.v3)
   throw Error('SWAP_EVENT_TOPIC_MISMATCH');
 const url=process.env.HTTPS_URL || 'https://ethereum-rpc.publicnode.com';
 if(!url.startsWith('https://'))throw Error('HTTPS_RPC_REQUIRED');
 const invPath=path.resolve(process.env.INVENTORY_PATH ||
   fileURLToPath(new URL('../evidence/run-26437145-full/inventory.json',import.meta.url)));
 const outPath=path.resolve(process.env.SAMPLE_OUTPUT ||
   fileURLToPath(new URL('../evidence/recent-swap-sample.json',import.meta.url)));
 const inv=JSON.parse(await readFile(invPath,'utf8')),index=buildPoolIndex(inv);
 const window=safeInteger(process.env.SWAP_WINDOW_BLOCKS,48,1,128);
 const limit=safeInteger(process.env.SWAP_MAX_CANDIDATES,8,1,12);
 const maxCalls=4+2*limit;let count=0;
 async function rpc(method,params){
   if(!['eth_chainId','eth_getBlockByNumber','eth_getLogs','eth_getTransactionReceipt'].includes(method))
     throw Error('NON_READ_ONLY_RPC_METHOD');
   if(++count>maxCalls)throw Error('RPC_BUDGET_EXCEEDED');
   const response=await fetch(url,{method:'POST',headers:{'content-type':'application/json'},
     body:JSON.stringify({jsonrpc:'2.0',id:count,method,params}),signal:AbortSignal.timeout(20000)});
   if(!response.ok)throw Error('RPC_HTTP_'+response.status+'_'+method);
   const obj=await response.json();
   if(obj.error||obj.result===null||obj.result===undefined)
     throw Error('RPC_METHOD_REJECTED_'+method);
   return obj.result;
 }
 const chainId=BigInt(await rpc('eth_chainId',[]));
 if(chainId!==1n)throw Error('WRONG_CHAIN');
 const inventoryHeader=await rpc('eth_getBlockByNumber',['0x'+index.blockNumber.toString(16),false]);
 if(inventoryHeader?.hash?.toLowerCase()!==index.blockHash.toLowerCase())
   throw Error('INVENTORY_BLOCK_HASH_MISMATCH');
 const end=await rpc('eth_getBlockByNumber',['finalized',false]);
 if(!/^0x[0-9a-f]{64}$/i.test(end.hash||''))throw Error('FINALIZED_HASH_MISSING');
 const endNumber=Number(BigInt(end.number));
 if(!Number.isSafeInteger(endNumber)||endNumber<index.blockNumber)
   throw Error('FINALIZED_BLOCK_BEFORE_POOL_INVENTORY');
 const from=Math.max(index.blockNumber,endNumber-window+1);
 const control=inv.markets.find(m=>m.control&&m.other?.toLowerCase()===
   '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48');
 if(!control)throw Error('NO_USDC_WETH_CONTROL');
 const selected=control.pools.filter(p=>index.pools.has(p.address.toLowerCase()) &&
   ['UniswapV2','UniswapV3'].includes(p.venue));
 if(selected.length<2)throw Error('INSUFFICIENT_CANONICAL_CONTROL_POOLS');
 const poolAddresses=selected.map(p=>p.address);
 const logs=await rpc('eth_getLogs',[{address:poolAddresses,
    fromBlock:'0x'+from.toString(16),toBlock:'0x'+endNumber.toString(16),
    topics:[[TOPICS.v2,TOPICS.v3]]}]);
 if(!Array.isArray(logs)||logs.length>5000)throw Error('UNBOUNDED_OR_INVALID_LOG_RESPONSE');
 const ranked=rankLargestSwaps(logs,index,{maxCandidates:limit,
    minWethWei:100000000000000000n,maxBlockNumber:endNumber});
 const confirmed=[],failed=[];
 for(const c of ranked.candidates) {
   try{
     const receipt=await rpc('eth_getTransactionReceipt',[c.txHash]);
     const block=await rpc('eth_getBlockByNumber',['0x'+c.blockNumber.toString(16),false]);
     confirmed.push(verifySelectedReceipt(c,receipt,block));
   }catch(e){
     failed.push({txHash:c.txHash,blockNumber:c.blockNumber,reason:String(e.message||e).slice(0,100)});
   }
 }
 const output={
   type:'INTRA_UNISWAP_SWAP_EVENT_PILOT',
   chainId:1,endBlockNumber:endNumber,endBlockHash:end.hash,
   inventoriedAtBlock:index.blockNumber,inventoryBlockHash:index.blockHash,
   queryRange:{from,to:endNumber,blocks:endNumber-from+1},
   poolAddresses,topics:[TOPICS.v2,TOPICS.v3],
   source:'RECENT_FINALIZED_RPC_LOGS_AND_RECEIPTS',provenance:process.env.HTTPS_URL?'PRIVATE_CONFIGURED_RPC':'PUBLICNODE_HTTPS',
   totalLogs:logs.length,rpcCalls:count,rpcBudget:maxCalls,
   selection:ranked.selection,candidates:confirmed,receiptFailures:failed,
   historicalUsdNotionalVerified:false,independentPoolDepthUsdVerified:false,
   transactionPositionReplay:'NOT_REPLAYED',
   executableRouteCount:0,verifiedPositiveNetTrades:0,
   codeDoesNotEstimateGasOrBuilderPayment:true,
   tradeExecutionAllowed:false,
   conclusion:confirmed.length?'SWAP_TRIGGERS_OBSERVED; REPLAY_ACCESS_REQUIRED':'NO_CONFIRMED_TRIGGERS_IN_BOUNDED_PILOT',
   caveats:[
     'WETH movement, not independently valued USD notional, is used for preliminary ordering.',
     'The snapshot pool identity is historical, not a re-certification of the current state.',
     'A receipt proves transaction inclusion and observed Swap, not the balances of other pools after the complete tx.',
     'No trades, quotes, gas-cost profitability or inclusion probability were evaluated.'
   ]
 };
 await mkdir(path.dirname(outPath),{recursive:true});
 await writeFile(outPath,JSON.stringify(output,null,2)+'\n');
 console.log(JSON.stringify({saved:outPath,block:endNumber,pools:poolAddresses.length,
   scannedLogs:logs.length,confirmed:confirmed.length,failed:failed.length,rpcCalls:count,
   netProfitVerified:0,transactionPositionReplay:'NOT_REPLAYED'},null,2));
}
main().catch(e=>{console.error('Read-only Swap sampler BLOCKED:',String(e.message||e).slice(0,160));process.exitCode=2;});
