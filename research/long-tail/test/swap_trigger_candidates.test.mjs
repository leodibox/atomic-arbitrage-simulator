import test from 'node:test';
import assert from 'node:assert/strict';
import {TOPICS,buildPoolIndex,parseSwap,rankLargestSwaps,verifySelectedReceipt} from '../src/swap_trigger_candidates.mjs';
const WETH='0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2';
const USDC='0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
const addr=n=>'0x'+n.toString(16).padStart(40,'0'), hash=n=>'0x'+n.toString(16).padStart(64,'0');
const word=x=>(x<0n?(1n<<256n)+x:x).toString(16).padStart(64,'0');
const pools=[
 {address:addr(1),kind:'v2',factory:'0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f',
  venue:'UniswapV2',fee:3000,status:'VERIFIED_IDENTITY',token0:USDC,token1:WETH,r0:'10000',r1:'10000'},
 {address:addr(2),kind:'v3',factory:'0x1F98431c8aD98523631AE4a59f267346ea31F984',
  venue:'UniswapV3',fee:100,status:'VERIFIED_IDENTITY',token0:USDC,token1:WETH,
  sqrtPriceX96:'79228162514264337593543950336',liquidity:'10000',unlocked:true}
];
const inventory=()=>({chainId:1,endBlock:{number:120,hash:hash(120)},markets:[{other:USDC,pools}]});
const v2=(tx=1,block=110,index=0,size=10n*10n**18n)=>({
  address:addr(1),blockNumber:'0x'+block.toString(16),blockHash:hash(block),
  transactionHash:hash(tx),transactionIndex:'0x0',logIndex:'0x'+index.toString(16),
  topics:[TOPICS.v2,hash(0),hash(0)],data:'0x'+word(5000n)+word(size)+word(0n)+word(0n)
});
const v3=(tx=2,block=111,index=1,delta=-12n*10n**18n)=>({
  address:addr(2),blockNumber:'0x'+block.toString(16),blockHash:hash(block),
  transactionHash:hash(tx),transactionIndex:'0x2',logIndex:'0x'+index.toString(16),
  topics:[TOPICS.v3,hash(0),hash(0)],data:'0x'+word(500000000n)+word(delta)+word(2n**96n)+word(1000n)+word(200000n)
});
test('WETH inventory recognizes two verified Uniswap pools',()=>{
 const x=buildPoolIndex(inventory());assert.equal(x.pools.size,2);assert.equal(x.blockNumber,120);
});
test('parse V2 WETH movement and V3 signed negative delta correctly',()=>{
 const ix=buildPoolIndex(inventory());
 assert.equal(parseSwap(v2(),ix).wethVolumeWei,(10n*10n**18n).toString());
 const x=parseSwap(v3(),ix);assert.equal(x.wethVolumeWei,(12n*10n**18n).toString());
 assert.equal(x.direction,'WETH_OUT');assert.equal(x.txIndex,2);
});
test('largest SINGLE Swap per block is selected with stable tie-breaking',()=>{
 const ix=buildPoolIndex(inventory());
 const x=rankLargestSwaps([v2(3,110,5,3n*10n**18n),v2(4,110,6,6n*10n**18n),v3(2,111,1,-12n*10n**18n)],ix);
 assert.equal(x.candidates.length,2);assert.equal(x.candidates[0].txHash,hash(2));
 assert.equal(x.candidates[1].txHash,hash(4));assert.equal(x.selection.distinctEligibleBlocks,2);
 assert.equal(x.verifiedPositiveNetTrades,0);
 assert.ok(x.candidates.every(c=>!c.tradeExecutionAllowed&&!c.postTransactionStateVerified));
});
test('a duplicate event is counted as rejected not counted twice',()=>{
 const ix=buildPoolIndex(inventory());
 const x=rankLargestSwaps([v2(),v2()],ix);assert.equal(x.selection.rejectedLogCount,1);
 assert.equal(x.selection.eligibleEvents,1);
});
test('foreign pool and wrong event type cannot be selected',()=>{
 const ix=buildPoolIndex(inventory());const foreign={...v2(),address:addr(9)};
 const wrong={...v3(),topics:[TOPICS.v2]};
 const x=rankLargestSwaps([foreign,wrong],ix);
 assert.equal(x.selection.rejectedLogCount,2);assert.equal(x.candidates.length,0);
});
test('receipt + block match only confirms transaction, NOT post-transaction state',()=>{
 const ix=buildPoolIndex(inventory());const e=v3();
 const x=rankLargestSwaps([e],ix).candidates[0];
 const receipt={transactionHash:x.txHash,blockHash:x.blockHash,
  transactionIndex:'0x2',blockNumber:'0x6f',status:'0x1',logs:[e]};
 const block={hash:x.blockHash,timestamp:'0x69000000',baseFeePerGas:'0x3b9aca00'};
 const checked=verifySelectedReceipt(x,receipt,block);
 assert.equal(checked.receiptVerified,true);assert.equal(checked.historicalBaseFeeWei,'1000000000');
 assert.equal(checked.replayStatus,'NOT_REPLAYED');
 assert.equal(checked.netProfitVerified,false);
});
test('receipt mismatch, reverted transaction, absent swap, or gasless header fails closed',()=>{
 const ix=buildPoolIndex(inventory()),e=v2(),x=rankLargestSwaps([e],ix).candidates[0];
 const receipt={transactionHash:x.txHash,blockHash:x.blockHash,
  transactionIndex:'0x0',blockNumber:'0x6e',status:'0x1',logs:[e]};
 const block={hash:x.blockHash,timestamp:'0x69000000',baseFeePerGas:'0x1'};
 assert.throws(()=>verifySelectedReceipt(x,{...receipt,status:'0x0'},block),/RECEIPT/);
 assert.throws(()=>verifySelectedReceipt(x,{...receipt,logs:[]},block),/TRIGGER_SWAP/);
 assert.throws(()=>verifySelectedReceipt(x,receipt,{...block,baseFeePerGas:null}),/BASE_FEE/);
 assert.throws(()=>verifySelectedReceipt(x,receipt,{...block,hash:hash(99)}),/RECEIPT/);
});
test('future logs cannot appear in a previous pinned inventory',()=>{
 const ix=buildPoolIndex(inventory());
 const x=rankLargestSwaps([v2(10,121)],ix);
 assert.equal(x.candidates.length,0);assert.equal(x.selection.rejections[0].reason,'SWAP_AFTER_CHECKPOINT');
});

test('recent Swap events after old inventory are accepted only under an explicit finalized bound',()=>{
 const ix=buildPoolIndex(inventory());
 const within=rankLargestSwaps([v2(10,121)],ix,{maxBlockNumber:121});
 assert.equal(within.candidates.length,1);
 const beyond=rankLargestSwaps([v2(10,122)],ix,{maxBlockNumber:121});
 assert.equal(beyond.candidates.length,0);
 assert.equal(beyond.selection.rejections[0].reason,'SWAP_AFTER_CHECKPOINT');
});
test('receipt with tampered Swap amount fails even with same tx hash/log index',()=>{
 const ix=buildPoolIndex(inventory()),e=v2();
 const candidate=rankLargestSwaps([e],ix).candidates[0];
 const altered={...e,data:'0x'+word(5000n)+word(99999999999999999999n)+word(0n)+word(0n)};
 const receipt={transactionHash:e.transactionHash,blockHash:e.blockHash,
   transactionIndex:'0x0',blockNumber:'0x6e',status:'0x1',logs:[altered]};
 const block={hash:e.blockHash,timestamp:'0x69000000',baseFeePerGas:'0x1'};
 assert.throws(()=>verifySelectedReceipt(candidate,receipt,block),/RECEIPT_SWAP_DATA_MISMATCH/);
});
test('spoofed factory or noncanonical fee cannot enter the pool index',()=>{
 const base=inventory();
 base.markets[0].pools=[{...pools[0],factory:addr(909)},{...pools[1],fee:999}];
 assert.equal(buildPoolIndex(base).pools.size,0);
});
