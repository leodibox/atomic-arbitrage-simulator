// Exactly two independent read-only probes, pinned to one old block.
import {tag,WETH} from './chain.mjs';
export async function diagnoseState(rpc,block){
 if(!Number.isSafeInteger(block)||block<0)throw Error('Invalid historical block');
 const run=async(method,params)=>{try{const raw=await rpc.call(method,params);if(typeof raw!=='string'||!/^0x[0-9a-f]{64}$/i.test(raw))throw Error('INVALID_ABI_WORD');return {method,status:'SUCCESS',block,raw};}catch(e){return {method,status:'FAILED',block,error:String(e.message).slice(0,100)};}};
 const ethCall=await run('eth_call',[{to:WETH,data:'0x18160ddd',gas:'0x186a0'},tag(block)]);
 const storage=await run('eth_getStorageAt',[WETH,'0x0',tag(block)]);
 return {type:'ISSUE15_HISTORICAL_STATE_CAPABILITY',block,contract:WETH,ethCall,storage,originalCoverageUpdated:false,approvedForExecution:false};
}
