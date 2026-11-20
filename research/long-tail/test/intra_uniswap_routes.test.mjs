import test from 'node:test';
import assert from 'node:assert/strict';
import {WETH,UNISWAP_FACTORIES,enumerateIntraUniswap,canonicalResearchPool} from '../src/intra_uniswap_routes.mjs';
const USDC='0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
const USDT='0xdAC17F958D2ee523a2206206994597C13D831ec7';
const addr=i=>'0x'+i.toString(16).padStart(40,'0');
function pool(i,a,b,{kind='v3',fee=500,venue, factory, inactive=false}={}){
 return {address:addr(i),token0:a,token1:b,kind,fee,
   factory:factory??UNISWAP_FACTORIES[kind],
   venue:venue??(kind==='v3'?'UniswapV3':'UniswapV2'),
   status:'VERIFIED_IDENTITY',
   ...(kind==='v3'?{sqrtPriceX96:'79228162514264337593543950336',
     liquidity:inactive?'0':'100000000000',unlocked:true}:
     {r0:'100000000000000',r1:'100000000000000'})};
}
const snap=markets=>({chainId:1,endBlock:{number:26437145,hash:'0x'+'ab'.repeat(32)},
  markets});
const market=(other,pools)=>({other,pools,tokenRisk:{safe:false,transferRisk:'UNKNOWN_NOT_TESTED'}});
const uniswap5=[
 pool(10,USDC,WETH,{kind:'v2',fee:3000}),
 pool(11,USDC,WETH,{fee:100}),
 pool(12,USDC,WETH,{fee:500}),
 pool(13,USDC,WETH,{fee:3000}),
 pool(14,USDC,WETH,{fee:10000})
];
test('5 distinct Uniswap pools produce 12 directed V3 tier + 8 V2/V3 routes',()=>{
 const result=enumerateIntraUniswap(snap([market(USDC,uniswap5)]));
 assert.equal(result.poolCount,5);
 assert.deepEqual(result.counts,{v3FeeTier:12,uniV2V3:8,triangles:0});
 assert.equal(result.routeCount,20);
 assert.ok(result.routes.every(r=>r.pools[0]!==r.pools[1] && r.tokens.length===3));
 assert.equal(result.actualProfitableTradeFound,false);
 assert.equal(result.transactionPositionReplay,'NOT_REPLAYED');
 assert.ok(result.routes.every(r=>!r.approvedForExecution&&!r.netProfitVerified&&!r.gasVerified));
});
test('single pool reversed is not arbitrage',()=>{
 const r=enumerateIntraUniswap(snap([market(USDC,[uniswap5[0]])]));
 assert.equal(r.routeCount,0);
});
test('Sushi pool excluded even when factory state and liquidity are positive',()=>{
 const sushi=pool(15,USDC,WETH,{kind:'v2',fee:3000,venue:'SushiV2',
    factory:'0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac'});
 const r=enumerateIntraUniswap(snap([market(USDC,[...uniswap5,sushi])]));
 assert.equal(r.poolCount,5);
 assert.equal(r.rejectedPoolObservations,1);
 assert.equal(r.routeCount,20);
});
test('one verified triangle exists in both directions, without same-token loops',()=>{
 const triangle=[
   pool(30,WETH,USDC,{fee:100}),
   pool(31,USDC,USDT,{fee:500}),
   pool(32,USDT,WETH,{kind:'v2',fee:3000})
 ];
 const r=enumerateIntraUniswap(snap([market(USDC,triangle)]));
 assert.equal(r.counts.triangles,2);
 assert.equal(r.counts.v3FeeTier,0);
 assert.ok(r.routes.every(x=>new Set(x.pools).size===3));
 assert.ok(r.routes.every(x=>new Set(x.tokens.slice(0,-1).map(v=>v.toLowerCase())).size===3));
});
test('duplicate inventory observations deduplicate identical pool identity',()=>{
 const p=uniswap5[0];
 const r=enumerateIntraUniswap(snap([market(USDC,[p,p])]));
 assert.equal(r.poolCount,1);
 assert.equal(r.routeCount,0);
});
test('unknown factories, unsupported fees, missing identity and inactive pools cannot enter topology',()=>{
 const invalid=[
  {...uniswap5[0],factory:addr(44)},
  {...uniswap5[1],fee:250},
  {...uniswap5[2],status:'MISSING'},
  {...uniswap5[3],liquidity:'0'}
 ];
 const r=enumerateIntraUniswap(snap([market(USDC,invalid)]));
 assert.equal(r.poolCount,0);
 assert.equal(r.rejectedPoolObservations,4);
});
test('invalid chain, block identity and path limit fail closed',()=>{
 assert.throws(()=>enumerateIntraUniswap({...snap([]),chainId:137}),/INVALID_INVENTORY/);
 assert.throws(()=>enumerateIntraUniswap({...snap([]),endBlock:{number:1,hash:'0xabc'}}),/INVALID_INVENTORY/);
 assert.throws(()=>enumerateIntraUniswap(snap([]),{maxRoutes:0}),/INVALID_ROUTE_LIMIT/);
 assert.throws(()=>canonicalResearchPool({}),/POOL_ADDRESS_OR_TOKENS_INVALID/);
});
test('route cap visibly truncates, does not pretend to cover the search space',()=>{
 const r=enumerateIntraUniswap(snap([market(USDC,uniswap5)]),{maxRoutes:3});
 assert.equal(r.routeCount,3);
 assert.equal(r.truncated,true);
});

test('conflicting pool identities at the same address abort the entire inventory',()=>{
 const original=uniswap5[0];
 const conflict={...original,token0:USDT};
 assert.throws(()=>enumerateIntraUniswap(
   snap([market(USDC,[original,conflict])])),/DUPLICATE_POOL_CONFLICT/);
});
