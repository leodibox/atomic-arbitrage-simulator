#!/usr/bin/env node
// Local-only route topology over previously verified PR #16 inventory.
// Does not access Ethereum, use a wallet, sign, send or claim executable profit.
import {readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {enumerateIntraUniswap} from './intra_uniswap_routes.mjs';

const checkpointPath=path.resolve(process.argv[2]||
  'evidence/run-26437145-full/inventory.json');
const outputPath=process.argv[3]?path.resolve(process.argv[3]):null;
try{
 const data=JSON.parse(await readFile(checkpointPath,'utf8'));
 const result=enumerateIntraUniswap(data);
 const evidence={
   ...result,explanation:'End-of-block route topology only; NOT a transaction-position replay, swap quote, after-gas PnL or trade signal.',
   routeSelectionPolicy:'Distinct canonical Uniswap V2/V3 pools; both directions; no Sushi and no same-pool reverse arbitrage.',
   historicalExecutionState:'INSUFFICIENT_REPLAY_ACCESS',
   triggerTransactionHash:null,triggerTransactionIndex:null,
   afterTriggerBalancesVerified:false,netAfterBuilderPaymentVerified:false
 };
 const output=JSON.stringify(evidence,null,2)+'\n';
 if(outputPath){await writeFile(outputPath,output);console.log('Saved topology:',outputPath);}
 else console.log(output);
 console.error('Topology only:',result.poolCount,'verified Uniswap pools;',
   result.routeCount,'routes; triangles=',result.counts.triangles,
   '; positive-net execution evidence=0');
}catch(e){
 console.error('Intra-Uniswap inventory inspection failed closed:',String(e.message||e).slice(0,200));
 process.exitCode=2;
}
