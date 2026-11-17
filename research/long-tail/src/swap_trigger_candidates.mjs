// Pure read-only selection logic for bounded Ethereum Swap-trigger research.
// This module NEVER claims a post-tx opportunity, trade signal or net EV.
export const TOPICS=Object.freeze({
  v2:'0xd78ad95fa46c994b6551d0da85fc275fe613d174a36d44dcaf13e0f521b7',
  v3:'0xc42079f94a6350d7e6235f2917495a9d78ff82dd33fbb8f9195e7590c9de8d3'
});
const WETH='0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2';
const fail=m=>{throw Error(m)};
const address=x=>typeof x==='string'&&/^0x[0-9a-f]{40}$/i.test(x);
const hash=x=>typeof x==='string'&&/^0x[0-9a-f]{64}$/i.test(x);
const eq=(a,b)=>a.toLowerCase()===b.toLowerCase();
const abs=x=>x>=0n?x:-x;
const big=x=>{try{return BigInt(x)}catch{fail('INVALID_HEX_QUANTITY')}};
function word(data,i){
 if(typeof data!=='string'||!/^0x[0-9a-f]+$/i.test(data)||data.length<2+(i+1)*64|| (data.length-2)%64!==0)
   fail('INVALID_EVENT_DATA');
 return BigInt('0x'+data.slice(2+i*64,2+(i+1)*64));
}
const signed=x=>x>=(1n<<255n)?x-(1n<<256n):x;
function intNumber(x) {
 const n=Number(big(x));
 if(!Number.isSafeInteger(n)||n<0)fail('INVALID_BLOCK_NUMBER');
 return n;
}
export function buildPoolIndex(inventory){
 if(inventory?.chainId!==1 || !Number.isSafeInteger(inventory?.endBlock?.number) ||
    !hash(inventory.endBlock.hash)||!Array.isArray(inventory.markets))
   fail('INVALID_PINNED_INVENTORY');
 const pools=new Map();
 for(const market of inventory.markets)for(const p of market.pools||[]) {
  if(!address(p.address)||!address(p.factory)||!address(p.token0)||!address(p.token1)||
      p.status!=='VERIFIED_IDENTITY'||!['v2','v3'].includes(p.kind)) continue;
  if(!eq(p.token0,WETH)&&!eq(p.token1,WETH))continue; // WETH-notional pilot only
  const k=p.address.toLowerCase(),simple={address:p.address,kind:p.kind,token0:p.token0,token1:p.token1,
    feePpm:p.fee,venue:p.venue,factory:p.factory};
  if(pools.has(k)){
   if(JSON.stringify(pools.get(k))!==JSON.stringify(simple))fail('CONFLICTING_POOL_IDENTITY');
  } else pools.set(k,simple);
 }
 return {blockNumber:inventory.endBlock.number,blockHash:inventory.endBlock.hash,pools};
}
export function parseSwap(log,index){
 if(!log||!address(log.address)||!hash(log.blockHash)||!hash(log.transactionHash)||
     !Array.isArray(log.topics) || log.removed===true)fail('INVALID_LOG_IDENTITY');
 const pool=index.pools.get(log.address.toLowerCase());
 if(!pool)fail('UNKNOWN_POOL');
 if(!eq(log.topics[0]||'',TOPICS[pool.kind]))fail('UNEXPECTED_SWAP_TOPIC');
 const words=pool.kind==='v2'?4:5;
 if(typeof log.data!=='string'||log.data.length!==2+words*64||!/^0x[0-9a-f]+$/i.test(log.data))
   fail('INVALID_EVENT_DATA');
 const weth0=eq(pool.token0,WETH);
 let size,direction;
 if(pool.kind==='v2'){
  const [a0in,a1in,a0out,a1out]=[0,1,2,3].map(i=>word(log.data,i));
  const got=weth0?a0in:a1in,sent=weth0?a0out:a1out;
  size=got>sent?got:sent;
  direction=got&&sent?'BIDIRECTIONAL_WETH_FLOW':got?'WETH_IN':sent?'WETH_OUT':'NO_WETH_FLOW';
 }else{
  const a0=signed(word(log.data,0)),a1=signed(word(log.data,1));
  const delta=weth0?a0:a1;
  size=abs(delta);
  direction=delta>0n?'WETH_IN':delta<0n?'WETH_OUT':'NO_WETH_FLOW';
 }
 return {poolAddress:pool.address,kind:pool.kind,feePpm:pool.feePpm,
   blockNumber:intNumber(log.blockNumber),blockHash:log.blockHash,
   txHash:log.transactionHash,txIndex:intNumber(log.transactionIndex),
   logIndex:intNumber(log.logIndex),wethVolumeWei:size.toString(),direction,
   triggeringPoolToken0:pool.token0,triggeringPoolToken1:pool.token1,
   candidateState:'SWAP_EVENT_ONLY_NOT_REPLAYED',tradeExecutionAllowed:false};
}
export function rankLargestSwaps(logs,index,{maxCandidates=12,minWethWei=1n}={}){
 if(!Number.isInteger(maxCandidates)||maxCandidates<1||maxCandidates>50||big(minWethWei)<0n)
   fail('INVALID_SELECTION_LIMITS');
 const events=[],rejections=[],seen=new Set();
 for(const log of logs){
  try{
   const v=parseSwap(log,index);
   if(v.blockNumber>index.blockNumber)fail('SWAP_AFTER_CHECKPOINT');
   const k=v.blockHash.toLowerCase()+':'+v.txHash.toLowerCase()+':'+v.logIndex;
   if(seen.has(k))fail('DUPLICATE_SWAP_LOG');
   seen.add(k);
   if(BigInt(v.wethVolumeWei)>=big(minWethWei))events.push(v);
  }catch(e){
   rejections.push({txHash:hash(log?.transactionHash)?log.transactionHash:null,
     address:address(log?.address)?log.address:null,reason:e.message});
  }
 }
 // Largest SINGLE swap per block, not the sum of multiple events inside one tx.
 const bestByBlock=new Map();
 for(const v of events){
  const key=v.blockHash.toLowerCase();
  const last=bestByBlock.get(key);
  if(!last || BigInt(v.wethVolumeWei)>BigInt(last.wethVolumeWei) ||
    BigInt(v.wethVolumeWei)===BigInt(last.wethVolumeWei) &&
    (v.txIndex<last.txIndex || v.txIndex===last.txIndex&&v.logIndex<last.logIndex))
    bestByBlock.set(key,v);
 }
 const selected=[...bestByBlock.values()].sort((a,b)=>
    BigInt(a.wethVolumeWei)>BigInt(b.wethVolumeWei)?-1:
    BigInt(a.wethVolumeWei)<BigInt(b.wethVolumeWei)?1:
    a.blockNumber-b.blockNumber||a.txIndex-b.txIndex||a.logIndex-b.logIndex);
 const candidates=selected.slice(0,maxCandidates).map(x=>({
   ...x,bluechipDepthIndependentlyVerified:false,
   usdNotionalVerified:false,receiptVerified:false,
   postTransactionStateVerified:false,coldGasVerified:false,
   netProfitVerified:false,builderPaymentVerified:false,
   pSuccessCalibrated:false,tradeExecutionAllowed:false
 }));
 return {candidates,selection:{
   methodology:'MAX_SINGLE_WETH_LEG_SWAP_PER_BLOCK',
   requestedCandidateLimit:maxCandidates,minWethWei:big(minWethWei).toString(),
   selected:candidates.length,eligibleEvents:events.length,distinctEligibleBlocks:bestByBlock.size,
   rejectedLogCount:rejections.length,rejections,
   completeness:'RECENT_WINDOW_PILOT_ONLY',
   notEquivalentToPreregistered30DayUsdStrata:true},
   profitabilityStatus:'NOT_REPLAYED',verifiedPositiveNetTrades:0};
}
export function verifySelectedReceipt(candidate,receipt,block){
 if(!candidate||!receipt||!block||!hash(block.hash) ||
    !eq(receipt.transactionHash||'',candidate.txHash)||
    !eq(receipt.blockHash||'',candidate.blockHash)||
    !eq(block.hash,candidate.blockHash) ||
    intNumber(receipt.transactionIndex)!==candidate.txIndex||
    intNumber(receipt.blockNumber)!==candidate.blockNumber||
    big(receipt.status)!==1n||!Array.isArray(receipt.logs))
   fail('RECEIPT_OR_HEADER_MISMATCH');
 const match=receipt.logs.find(l=>
   address(l.address)&&eq(l.address,candidate.poolAddress)&&
   intNumber(l.logIndex)===candidate.logIndex&&
   eq(l.topics?.[0]||'',TOPICS[candidate.kind]));
 if(!match)fail('TRIGGER_SWAP_NOT_IN_RECEIPT');
 const fee=block.baseFeePerGas;
 if(fee===undefined||fee===null)fail('HISTORICAL_BASE_FEE_MISSING');
 return {...candidate,receiptVerified:true,historicalBaseFeeWei:big(fee).toString(),
   blockTimestamp:intNumber(block.timestamp),
   triggerTransactionCompleted:true,
   afterTriggerPosition:'AFTER_FULL_TX_'+candidate.txIndex,
   postTransactionStateVerified:false,
   replayStatus:'NOT_REPLAYED',netProfitVerified:false,tradeExecutionAllowed:false};
}
