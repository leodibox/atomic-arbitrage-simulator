// Pure selection and Geth boundary planning. No network or sender.
export function selectFirstSwap(sample){
 if(sample?.chainId!==1||sample.totalLogs!==166||sample.rpcCalls!==20||sample.candidates?.length!==8||sample.receiptFailures?.length!==0)throw Error('PRESERVED_SAMPLE_MISMATCH');
 const c=[...sample.candidates];
 if(c.some(x=>!x.receiptVerified||!x.triggerTransactionCompleted||x.tradeExecutionAllowed!==false||!Number.isSafeInteger(x.txIndex)||x.txIndex<0))throw Error('UNVERIFIED_CANDIDATE');
 c.sort((a,b)=>BigInt(a.wethVolumeWei)>BigInt(b.wethVolumeWei)?-1:BigInt(a.wethVolumeWei)<BigInt(b.wethVolumeWei)?1:a.blockNumber-b.blockNumber||a.txIndex-b.txIndex||a.logIndex-b.logIndex);
 return {...c[0],selectionRule:'descending verified single Swap WETH leg, then block/transaction/log index ascending',profitabilityInferred:false};
}
export function gethAfterTransactionPlan(candidate,block){
 if(block.hash?.toLowerCase()!==candidate.blockHash.toLowerCase()||Number(BigInt(block.number))!==candidate.blockNumber||block.transactions?.[candidate.txIndex]?.toLowerCase()!==candidate.txHash.toLowerCase())throw Error('CANONICAL_TX_POSITION_MISMATCH');
 const next=candidate.txIndex+1;
 if(next>=block.transactions.length)throw Error('NEXT_TRANSACTION_BOUNDARY_NOT_AVAILABLE');
 // Geth StateAtTransaction returns BEFORE txIndex. Therefore +1 is after the full trigger.
 return {method:'debug_traceCall',params:[{to:candidate.poolAddress,data:'0x3850c7bd',gas:'0x186a0'},candidate.blockHash,
  {tracer:'callTracer',timeout:'5s',reexec:64,txIndex:'0x'+next.toString(16)}],
  triggerTransactionIndex:candidate.txIndex,requestedPreTransactionIndex:next,
  boundary:'AFTER_COMPLETE_TRIGGER_BEFORE_NEXT_TRANSACTION',endOfBlockSubstitute:false,
  semanticsSource:'Geth eth/tracers/api.go -> eth/state_accessor.go StateAtTransaction returns before target index',
  poolStateVerified:false};
}
