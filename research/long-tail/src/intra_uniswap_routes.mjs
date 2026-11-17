// Research-only intra-Uniswap route topology from an existing pinned inventory.
// No transactions, network I/O, price quotes or profitability claims.
export const WETH = '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2';
export const UNISWAP_FACTORIES = Object.freeze({
  v2: '0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f',
  v3: '0x1F98431c8aD98523631AE4a59f267346ea31F984'
});
const isAddress = x => typeof x === 'string' && /^0x[0-9a-f]{40}$/i.test(x);
const lower = x => x.toLowerCase();
const fail = msg => { throw Error(msg); };
const isPositive = value => {try{return BigInt(value)>0n;}catch{return false;}};

export function canonicalResearchPool(p) {
  if(!p || !isAddress(p.address) || !isAddress(p.factory) ||
     !isAddress(p.token0) || !isAddress(p.token1) ||
     lower(p.token0)===lower(p.token1)) fail('POOL_ADDRESS_OR_TOKENS_INVALID');
  if(p.status!=='VERIFIED_IDENTITY') fail('POOL_IDENTITY_UNVERIFIED');
  if(!['v2','v3'].includes(p.kind) ||
     lower(p.factory)!==lower(UNISWAP_FACTORIES[p.kind]) ||
     p.venue!==(p.kind==='v2'?'UniswapV2':'UniswapV3')) fail('POOL_FACTORY_NOT_UNISWAP');
  if(!Number.isInteger(p.fee) ||
     (p.kind==='v2'&&p.fee!==3000) ||
     (p.kind==='v3'&&![100,500,3000,10000].includes(p.fee)))
     fail('POOL_FEE_UNSUPPORTED');
  if(p.kind==='v2'&&(!isPositive(p.r0)||!isPositive(p.r1)))
    fail('V2_RESERVES_MISSING');
  if(p.kind==='v3'&&(!isPositive(p.sqrtPriceX96) || !isPositive(p.liquidity) || p.unlocked!==true))
    fail('V3_STATE_UNAVAILABLE');
  return {address:p.address,token0:p.token0,token1:p.token1,
    kind:p.kind,venue:p.venue,feePpm:p.fee,feeBps:p.fee/100,factory:p.factory};
}

export function enumerateIntraUniswap(inventory,{anchor=WETH,maxRoutes=5000}={}) {
  if(!isAddress(anchor))fail('INVALID_ANCHOR');
  if(!Number.isInteger(maxRoutes)||maxRoutes<1||maxRoutes>50000)fail('INVALID_ROUTE_LIMIT');
  if(inventory?.chainId!==1 || !Number.isSafeInteger(inventory.endBlock?.number) ||
     !/^0x[0-9a-f]{64}$/i.test(inventory.endBlock?.hash||'') ||
     !Array.isArray(inventory.markets))fail('INVALID_INVENTORY_SNAPSHOT');
  // Only states whose on-chain identity was checked are admitted.
  const byAddress=new Map(), errors=[];
  for(const market of inventory.markets)for(const source of market.pools||[]) {
    try {
      const p=canonicalResearchPool(source),k=lower(p.address);
      if(byAddress.has(k)){
        const original=byAddress.get(k);
        if(JSON.stringify(original)!==JSON.stringify(p)) fail('DUPLICATE_POOL_CONFLICT');
      }else byAddress.set(k,p);
    }catch(e){if(e.message==='DUPLICATE_POOL_CONFLICT')throw e;
      errors.push({address:source?.address||null,reason:e.message});}
  }
  const pools=[...byAddress.values()];
  const adjacency=new Map();
  function add(from,to,pool){
    const key=lower(from);
    if(!adjacency.has(key))adjacency.set(key,[]);
    adjacency.get(key).push({from,to,pool});
  }
  for(const p of pools){add(p.token0,p.token1,p);add(p.token1,p.token0,p);}
  for(const edges of adjacency.values())edges.sort((a,b)=>lower(a.pool.address).localeCompare(lower(b.pool.address))||lower(a.to).localeCompare(lower(b.to)));
  const routes=[], seen=new Set();
  let truncated=false;
  const base=lower(anchor);
  function record(edges,kind){
    const key=edges.map(e=>lower(e.pool.address)+':'+lower(e.from)+':'+lower(e.to)).join('|');
    if(seen.has(key))return;
    seen.add(key);
    if(routes.length>=maxRoutes){truncated=true;return;}
    routes.push({
      type:kind,
      start:anchor,
      tokens:[anchor,...edges.map(e=>e.to)],
      pools:edges.map(e=>e.pool.address),
      poolKinds:edges.map(e=>e.pool.kind),
      feePpm:edges.map(e=>e.pool.feePpm),
      totalFeeBpsApprox:edges.reduce((n,e)=>n+e.pool.feeBps,0),
      blockNumber:inventory.endBlock.number,
      blockHash:inventory.endBlock.hash,
      statePosition:'END_OF_BLOCK_SNAPSHOT',
      transactionPositionVerified:false,
      exactQuoteVerified:false,
      gasVerified:false,
      netProfitVerified:false,
      fundingVerified:false,
      settlementVerified:false,
      approvedForExecution:false
    });
  }
  function walk(token,steps,usedPools,seenTokens){
    if(steps.length>=3)return;
    for(const edge of adjacency.get(lower(token))||[]){
      const poolKey=lower(edge.pool.address),nextKey=lower(edge.to);
      if(usedPools.has(poolKey))continue;
      if(nextKey===base){
        if(steps.length===1){
          const full=[...steps,edge];
          const a=full[0].pool,b=full[1].pool;
          if(a.kind==='v3'&&b.kind==='v3'&&a.feePpm!==b.feePpm)
            record(full,'V3_FEE_TIER_TWO_POOL');
          else if(a.kind!==b.kind)
            record(full,'UNISWAP_V2_V3_TWO_POOL');
        }
        if(steps.length===2)record([...steps,edge],'UNISWAP_THREE_HOP');
      }else if(!seenTokens.has(nextKey)&&steps.length<2){
        walk(edge.to,[...steps,edge],new Set([...usedPools,poolKey]),
             new Set([...seenTokens,nextKey]));
      }
    }
  }
  walk(anchor,[],new Set(),new Set([base]));
  routes.sort((a,b)=>a.totalFeeBpsApprox-b.totalFeeBpsApprox ||
    a.type.localeCompare(b.type) || a.pools.join(',').localeCompare(b.pools.join(',')) ||
    a.tokens.join(',').localeCompare(b.tokens.join(',')));
  return {
    chainId:1,blockNumber:inventory.endBlock.number,blockHash:inventory.endBlock.hash,
    source:'PR16_PINNED_END_OF_BLOCK_INVENTORY',poolCount:pools.length,
    rejectedPoolObservations:errors.length,errors,
    routes,routeCount:routes.length,truncated,
    counts:{
      v3FeeTier:routes.filter(r=>r.type==='V3_FEE_TIER_TWO_POOL').length,
      uniV2V3:routes.filter(r=>r.type==='UNISWAP_V2_V3_TWO_POOL').length,
      triangles:routes.filter(r=>r.type==='UNISWAP_THREE_HOP').length
    },
    transactionPositionReplay:'NOT_REPLAYED',
    actualProfitableTradeFound:false,approvedForExecution:false
  };
}
