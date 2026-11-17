import test from 'node:test';
import assert from 'node:assert/strict';
import {locateGap,diagnoseRpc} from '../src/rpc_capability.mjs';
import {FACTORIES} from '../src/chain.mjs';

const factory=FACTORIES.find(x=>x.name==='UniswapV2');
const example=()=>({
  chainId:1,endBlock:{number:30,hash:'0x'+'ab'.repeat(32)},discovery:[{
    factory:{...factory},events:[],coverage:[
      {from:1,to:20,status:'FAILED',reason:'RPC_HTTP_403'},
      {from:21,to:30,status:'COMPLETE',logs:1}
    ]
  }]
});
function rpcMock({oldError='RPC_HTTP_403',oldTenError=null,oldWindowError=null,
  recentError=null,wrongChain=false,wrongHash=false}={}){
  const calls=[];
  return {
    calls,
    async call(method,args){
      calls.push([method,args]);
      if(method==='eth_chainId')return wrongChain?'0x89':'0x1';
      if(method==='eth_getBlockByNumber')return {
        hash:'0x'+(wrongHash?'cd':'ab').repeat(32),number:'0x1e'
      };
      if(method==='eth_getLogs'){
        const f=Number(BigInt(args[0].fromBlock)),t=Number(BigInt(args[0].toBlock));
        if(f>=21&&recentError)throw Error(recentError);
        if(f<=20){
          const reason=f===20&&t===20?oldError:
            t-f+1<=10?oldTenError:oldWindowError;
          if(reason)throw Error(reason);
        }
        return [{blockNumber:args[0].fromBlock}];
      }
      throw Error('Unexpected method');
    }
  };
}
test('finds exact denial boundary; refuses incomplete coverage accounting',()=>{
 const gap=locateGap(example().discovery,'UniswapV2');
 assert.deepEqual([gap.olderTarget,gap.acceptedTarget,gap.failedBlocks,gap.completedBlocks],
  [20,21,20,10]);
 const x=example();x.discovery[0].coverage[1].from=22;
 assert.throws(()=>locateGap(x.discovery,'UniswapV2'),/gaps or overlaps/);
});
test('one-block 403: cannot claim shrinking ranges will fix historical access',async()=>{
 const rpc=rpcMock();
 const x=await diagnoseRpc(rpc,example());
 assert.equal(x.finding,'SINGLE_OLD_BLOCK_ACCESS_DENIED');
 assert.equal(x.probes.oldTenBlocks.status,'NOT_TESTED');
 assert.equal(x.probes.oldOriginalWindow.status,'NOT_TESTED');
 assert.equal(x.originalCoverageUpdated,false);
 assert.equal(x.approvedForExecution,false);
 assert.equal(rpc.calls.length,4);
});
test('old one block and ten work but old full window fails: range restriction',async()=>{
 const rpc=rpcMock({oldError:null,oldWindowError:'RPC_HTTP_403'});
 const x=await diagnoseRpc(rpc,example());
 assert.equal(x.finding,'RANGE_OR_PROVIDER_LIMIT_AT_TWO_THOUSAND_BLOCKS');
 assert.equal(x.probes.oldOneBlock.status,'SUCCESS');
 assert.equal(x.probes.oldTenBlocks.status,'SUCCESS');
 assert.equal(x.probes.oldOriginalWindow.status,'FAILED');
 assert.equal(rpc.calls.length,6);
});
test('old 10 block fails even when one block succeeds',async()=>{
 const x=await diagnoseRpc(rpcMock({oldError:null,oldTenError:'RPC_HTTP_403'}),example());
 assert.equal(x.finding,'RANGE_OR_PROVIDER_LIMIT_AT_TEN_BLOCKS');
 assert.equal(x.probes.oldOriginalWindow.status,'NOT_TESTED');
});
test('full old window recovered: marked possibly transient, not coverage repaired',async()=>{
 const x=await diagnoseRpc(rpcMock({oldError:null}),example());
 assert.equal(x.finding,'OLD_WINDOW_NOW_AVAILABLE');
 assert.equal(x.originalCoverageUpdated,false);
});
test('recent check failed makes diagnostic inconclusive',async()=>{
 const x=await diagnoseRpc(rpcMock({recentError:'RPC_HTTP_403'}),example());
 assert.equal(x.finding,'INCONCLUSIVE_RECENT_PROBE_FAILED');
});
test('fail closed on wrong chain, block hash or factory address',async()=>{
 await assert.rejects(()=>diagnoseRpc(rpcMock({wrongChain:true}),example()),/Wrong network/);
 await assert.rejects(()=>diagnoseRpc(rpcMock({wrongHash:true}),example()),/hash mismatch/);
 const x=example();x.discovery[0].factory.address='0x1111111111111111111111111111111111111111';
 await assert.rejects(()=>diagnoseRpc(rpcMock(),x),/Factory mismatch/);
});
