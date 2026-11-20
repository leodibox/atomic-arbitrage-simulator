import {mkdir,writeFile,readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {Rpc,FACTORIES,WETH,USDC,MORPHO,AAVE,QUOTER,abi,tag,same,lower,mapLimit,findStart,scanEvents,tokenRisk,discoverExisting,state,oriented,quote,routeMarginal} from './chain.mjs';
import {E18,plain,v2Sizing,gasScenarios,ceil} from './math.mjs';
export function selectStratified(events,max){const sorted=[...events].sort((a,b)=>a.blockNumber-b.blockNumber||lower(a.other).localeCompare(lower(b.other)));if(sorted.length<=max)return sorted;return Array.from({length:max},(_,i)=>sorted[Math.floor(i*(sorted.length-1)/(max-1))]);}
export async function probe(){
 const rpc=new Rpc(process.env.HTTPS_URL,{maxCalls:Number(process.env.MAX_RPC_CALLS||6000)});
 const maxPairs=Number(process.env.MAX_PAIRS||80),maxRoutes=Number(process.env.MAX_SIZED_ROUTES||20);
 if(!Number.isInteger(maxPairs)||maxPairs<2||maxPairs>200||!Number.isInteger(maxRoutes)||maxRoutes<1||maxRoutes>100)throw Error('invalid resource bounds');
 if(BigInt(await rpc.call('eth_chainId',[]))!==1n)throw Error('WRONG_CHAIN');
 const checkpoint=process.env.DISCOVERY_CHECKPOINT?JSON.parse(await readFile(process.env.DISCOVERY_CHECKPOINT,'utf8')):null;
 const end=await rpc.call('eth_getBlockByNumber',[checkpoint?tag(checkpoint.endBlock.number):'finalized',false]),H=Number(BigInt(end.number));
 if(checkpoint&&(checkpoint.chainId!==1||checkpoint.endBlock.hash!==end.hash))throw Error('CHECKPOINT_CHAIN_OR_HASH_MISMATCH');
 const output=path.resolve(process.env.RUN_DIR||fileURLToPath(new URL('../evidence/run-'+H,import.meta.url)));await mkdir(output,{recursive:true});
 const save=async(name,data)=>writeFile(path.join(output,name),JSON.stringify(plain(data),null,2)+'\n');
 const result={issue:15,chainId:1,endBlock:{number:H,hash:end.hash,timestamp:Number(BigInt(end.timestamp))},source:'HTTPS RPC process binding; public fallback provenance recorded by operator, URL/keys not stored',limits:{maxPairs,maxSizedRoutes:maxRoutes,maxRpcCalls:rpc.maxCalls,eventChunkBlocks:2000,anchorAsset:WETH,historicalVenueLookup:'UniswapV2/SushiV2 and all 4 canonical UniswapV3 fee tiers'},executable:false,errors:[]};
 console.error('Selected finalized block',H);
 for(const f of FACTORIES)await rpc.code(f.address,H);await rpc.code(WETH,H);await rpc.code(MORPHO,H);await rpc.code(AAVE,H);await rpc.code(QUOTER,H);
 result.startBlock=checkpoint?checkpoint.startBlock:await findStart(rpc,H,result.endBlock.timestamp-7*86400);
 const startHeader=await rpc.call('eth_getBlockByNumber',[tag(result.startBlock),false]);result.startHeader={number:result.startBlock,hash:startHeader.hash,timestamp:Number(BigInt(startHeader.timestamp))};
 result.discovery=checkpoint?checkpoint.discovery:[];
 result.discoveryReused=!!checkpoint;
 if(!checkpoint)for(const f of FACTORIES){console.error('Scanning',f.name,result.startBlock,H);result.discovery.push(await scanEvents(rpc,f,result.startBlock,H));await save('discovery.json',result);}
 const all=result.discovery.flatMap(x=>x.events);const unique=new Map();let nonWeth=0;
 for(const e of all){if(!same(e.token0,WETH)&&!same(e.token1,WETH)){nonWeth++;continue;}const other=same(e.token0,WETH)?e.token1:e.token0;const k=lower(other);if(!unique.has(k))unique.set(k,{other,blockNumber:e.blockNumber,newPool:e.address});}
 const selected=selectStratified([...unique.values()],maxPairs);if(!selected.some(x=>same(x.other,USDC)))selected.push({other:USDC,blockNumber:H,control:true});else selected.find(x=>same(x.other,USDC)).control=true;
 result.coverage={newPoolEvents:all.length,distinctNewPoolAddresses:new Set(all.map(e=>lower(e.address))).size,uniqueAllTokenPairs:new Set(all.map(e=>[lower(e.token0),lower(e.token1)].sort().join('/'))).size,wethAnchoredPairs:unique.size,nonWethEventsExcluded:nonWeth,selectedLongTailPairs:selected.filter(x=>!x.control).length,controlAdded:selected.some(x=>x.control),sampling:'deterministic stratified by creation block across complete discovered WETH pair set; not economic cherry-picking',eventCoverageComplete:result.discovery.every(x=>x.coverage.every(c=>c.status==='COMPLETE'))};
 const cash=(await rpc.read(WETH,'balanceOf',[MORPHO],H))[0],premium=(await rpc.read(AAVE,'FLASHLOAN_PREMIUM_TOTAL',[],H))[0];
 result.funding={morpho:{cashWeth:cash,feeBps:0,feeSource:'canonical Morpho implementation and prior scoped fork behavior; no fee getter',availability:'OBSERVED_SINGLETON_CASH_NOT_CURRENT_LOAN_PROOF'},aave:{premiumBps:premium,availability:'UNKNOWN'}};
 try{const raw=await rpc.call('eth_call',[{to:AAVE,data:abi.encodeFunctionData('getReserveData',[WETH]),gas:'0x7a120'},tag(H)]);if(raw.length<2+9*64)throw Error('reserve data short');const at='0x'+raw.slice(2+8*64+24,2+9*64);const conf=(await rpc.read(AAVE,'getConfiguration',[WETH],H))[0];result.funding.aave={premiumBps:premium,aToken:at,cashWeth:(await rpc.read(WETH,'balanceOf',[at],H))[0],active:!!((conf>>56n)&1n),paused:!!((conf>>60n)&1n),flashLoanEnabled:!!((conf>>63n)&1n),availability:'FLAGS_AND_CASH_ONLY_NOT_LOAN_PROOF'};}catch(e){result.funding.aave.error=e.message;}
 console.error('Event discovery complete:',all.length,'events;',selected.length,'pairs selected for identity checks');
 result.markets=await mapLimit(selected,2,async(candidate,i)=>{
  let risk;try{risk=await tokenRisk(rpc,candidate.other,H);}catch(e){return {...candidate,error:e.message,stage:'TOKEN_METADATA',tokenRisk:{safe:false,transferRisk:'UNKNOWN_NOT_TESTED'}};}
  const inventory=await discoverExisting(rpc,candidate.other,H);
  const states=await mapLimit(inventory.pools,2,p=>state(rpc,p,H));
  const pools=states.filter(x=>x.address&&!x.error);
  const m={...candidate,tokenRisk:risk,inventoryChecks:inventory.checks,pools,stateErrors:states.filter(x=>x.error)};
  console.error('Market',i+1,'/',selected.length,'verified pools',pools.length);
  return m;
 });
 await save('inventory.json',result);
 const routes=[];
 for(const m of result.markets){if(m.error||m.pools.length<2)continue;for(const first of m.pools)for(const second of m.pools){if(same(first.address,second.address))continue;const spread=routeMarginal(first,second);routes.push({other:m.other,control:!!m.control,first,second,marginal:spread,spreadAfterFeesBps:((spread.n-spread.d)*1000000n/spread.d).toString(),risk:m.tokenRisk});}}
 // spread field scaled 1e6 of unit return; divide by 100 to obtain bps.
 routes.sort((a,b)=>{const x=a.marginal.n*b.marginal.d-b.marginal.n*a.marginal.d;return x>0n?-1:x<0n?1:0;});
 result.routeFilters=routes.map(x=>({other:x.other,first:x.first.address,second:x.second.address,control:x.control,marginal:x.marginal,spreadAfterFeesBpsHundredths:x.spreadAfterFeesBps,reason:x.marginal.n>x.marginal.d?'POSITIVE_MARGINAL_NOT_PROFIT_PROOF':'NONPOSITIVE_MARGINAL_FILTER',quote_verified:false,gas_verified:false,executable:false}));
 const controls=routes.filter(x=>x.control).slice(0,2),longtail=routes.filter(x=>!x.control).slice(0,Math.max(0,maxRoutes-controls.length));const sized=[...longtail,...controls];
 result.observationBlocks=[];
 for(const b of [H-2,H-1,H]){const header=await rpc.call('eth_getBlockByNumber',[tag(b),false]);result.observationBlocks.push({number:b,hash:header.hash,timestamp:Number(BigInt(header.timestamp))});}
 result.observations=[];
 for(const block of result.observationBlocks){console.error('Sizing at pinned block',block.number,'routes',sized.length);
  for(const route of sized){const row={block,other:route.other,control:route.control,first:route.first.address,second:route.second.address,firstKind:route.first.kind,secondKind:route.second.kind,tokenRisk:route.risk,quote_verified:false,gas_verified:false,executable:false,points:[]};
   try{const first=await state(rpc,route.first,block.number),second=await state(rpc,route.second,block.number);row.states={first,second};
    const morphoCash=(await rpc.read(WETH,'balanceOf',[MORPHO],block.number))[0],p=(await rpc.read(AAVE,'FLASHLOAN_PREMIUM_TOTAL',[],block.number))[0];row.funding={morphoCash,premiumBps:p,morphoAvailability:'cash only; fork loan proof pending'};
    let cap=3n*E18;if(cap>morphoCash)cap=morphoCash;
    for(const pool of [first,second])if(pool.kind==='v2'&&cap>pool.wethDepth/10n)cap=pool.wethDepth/10n;
    row.sizingCapRaw=cap;let sizes=[10n**15n,3n*10n**15n,10n**16n,3n*10n**16n,10n**17n,3n*10n**17n,E18,3n*E18].filter(x=>x<=cap);
    if(first.kind==='v2'&&second.kind==='v2'){
     row.v2Sizing=[];
     for(const fee of [0n,p])for(const gwei of [10n,30n,60n]){const model=v2Sizing(oriented(first,WETH),oriented(second,route.other),fee,500000n*gwei*10n**9n,cap);row.v2Sizing.push({premiumBps:fee,gasPriceGwei:gwei,...model});if(model.optimalRaw>0n)sizes.push(model.optimalRaw);}
     row.v2AnalyticAssumptions='static constant-product reserves, canonical 30bps, no transfer/rebase anomalies';
    }else row.v3Sizing='bounded exact-quote grid; no global optimum or continuous break-even asserted across ticks';
    sizes=[...new Set(sizes.map(String))].map(BigInt).sort((a,b)=>a<b?-1:1);
    for(const amount of sizes){try{const q1=await quote(rpc,first,WETH,amount,block.number);if(q1.out===0n)throw Error('ZERO_INTERMEDIATE');const q2=await quote(rpc,second,route.other,q1.out,block.number);const margin=q2.out-amount;row.points.push({amountWei:amount,intermediateRaw:q1.out,outputWei:q2.out,marginBeforeFundingWei:margin,firstQuote:q1,secondQuote:q2,quote_verified:first.kind==='v3'||second.kind==='v3',transferSettlementVerified:false,
     funding:[{provider:'Morpho',premiumWei:0n,marginBeforeGasWei:margin,scenarios:gasScenarios(500000n,margin)},{provider:'Aave',premiumWei:ceil(amount*p,10000n),marginBeforeGasWei:margin-ceil(amount*p,10000n),scenarios:gasScenarios(500000n,margin-ceil(amount*p,10000n))}],executable:false});}catch(e){row.points.push({amountWei:amount,error:e.message,executable:false});}}
    row.quote_verified=row.points.some(x=>x.quote_verified);row.bestSample=row.points.filter(x=>!x.error).sort((a,b)=>a.marginBeforeFundingWei>b.marginBeforeFundingWei?-1:1)[0]??null;
   }catch(e){row.error=e.message;}
   result.observations.push(row);await save('results.json',result);
  }
 }
 result.summary={newPools:result.coverage.distinctNewPoolAddresses,uniquePairs:result.coverage.uniqueAllTokenPairs,verifiedMarketsWithTwoPools:result.markets.filter(m=>!m.error&&m.pools.length>=2).length,orderedRoutes:routes.length,sizedRoutes:sized.length,observationBlocks:result.observationBlocks.length,routeObservations:result.observations.length,positiveQuotedPointsAfter10Gwei:result.observations.flatMap(o=>o.points).filter(x=>!x.error&&x.funding[0].scenarios[0].postGasWei>0n).length,positiveSafetyVerified:0,decision:'NO-GO',decisionReason:'No safety/funding/fork-certified positive route established; raw positive quotes never authorize GO',rpcCalls:rpc.count,executable:false};
 await save('results.json',result);
 const selectedFork=result.observations.filter(x=>x.block.number===H&&x.bestSample).sort((a,b)=>a.bestSample.marginBeforeFundingWei>b.bestSample.marginBeforeFundingWei?-1:1).slice(0,3).map(o=>({blockNumber:H,blockHash:end.hash,other:o.other,first:o.states.first,second:o.states.second,amountWei:o.bestSample.amountWei,expectedOutputWei:o.bestSample.outputWei,marginBeforeGasWei:o.bestSample.marginBeforeFundingWei,tokenRisk:o.tokenRisk,executable:false}));
 await save('fork-candidates.json',selectedFork);console.error('DONE',JSON.stringify(plain(result.summary)),output);return result;
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url))probe().catch(e=>{console.error('Probe failed closed:',e.message);process.exitCode=1;});
