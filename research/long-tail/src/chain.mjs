import {Interface,id} from 'ethers';
export const WETH='0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',USDC='0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
export const MORPHO='0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb',AAVE='0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2';
export const QUOTER='0x61fFE014bA17989E743c5F6cB21bF9697530B21e';
export const FACTORIES=[
 {name:'UniswapV2',kind:'v2',address:'0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f'},
 {name:'SushiV2',kind:'v2',address:'0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac'},
 {name:'UniswapV3',kind:'v3',address:'0x1F98431c8aD98523631AE4a59f267346ea31F984'}];
export const abi=new Interface([
 'function getPair(address,address) view returns(address)','function getPool(address,address,uint24) view returns(address)',
 'function token0() view returns(address)','function token1() view returns(address)','function factory() view returns(address)',
 'function fee() view returns(uint24)','function tickSpacing() view returns(int24)',
 'function getReserves() view returns(uint112,uint112,uint32)','function liquidity() view returns(uint128)',
 'function slot0() view returns(uint160,int24,uint16,uint16,uint16,uint8,bool)',
 'function decimals() view returns(uint8)','function balanceOf(address) view returns(uint256)',
 'function owner() view returns(address)','function paused() view returns(bool)',
 'function FLASHLOAN_PREMIUM_TOTAL() view returns(uint128)','function getReserveData(address) view returns(bytes32)',
 'function getConfiguration(address) view returns(uint256)',
 'function quoteExactInputSingle((address tokenIn,address tokenOut,uint256 amountIn,uint24 fee,uint160 sqrtPriceLimitX96)) returns(uint256,uint160,uint32,uint256)',
 'event PairCreated(address indexed token0,address indexed token1,address pair,uint256)',
 'event PoolCreated(address indexed token0,address indexed token1,uint24 indexed fee,int24 tickSpacing,address pool)']);
export const lower=x=>x.toLowerCase(),same=(a,b)=>lower(a)===lower(b),tag=n=>'0x'+BigInt(n).toString(16);
export const TOPICS={v2:id('PairCreated(address,address,address,uint256)'),v3:id('PoolCreated(address,address,uint24,int24,address)')};
export class Rpc {
 constructor(url,{maxCalls=6000}={}){if(!/^https:\/\//i.test(url||''))throw Error('secure HTTPS_URL required');this.url=url;this.maxCalls=maxCalls;this.count=0;this.errors=[];}
 async call(method,params){
  if(!['eth_chainId','eth_getBlockByNumber','eth_getLogs','eth_getCode','eth_call','eth_getStorageAt'].includes(method))throw Error('non-read-only method forbidden');
  if(++this.count>this.maxCalls)throw Error('RPC_BUDGET_EXHAUSTED');
  let r;try{r=await fetch(this.url,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:this.count,method,params}),signal:AbortSignal.timeout(25000)});}catch{throw Error('RPC_NETWORK_OR_TIMEOUT');}
  if(!r.ok)throw Error('RPC_HTTP_'+r.status);
  let j;try{j=await r.json();}catch{throw Error('RPC_INVALID_JSON');}
  if(j.error)throw Error('RPC_ERROR_'+j.error.code);if(j.result==null)throw Error('RPC_NULL_RESULT');return j.result;
 }
 async read(to,fn,args,block){const raw=await this.call('eth_call',[{to,data:abi.encodeFunctionData(fn,args),gas:'0x7a120'},tag(block)]);return abi.decodeFunctionResult(fn,raw);}
 async code(to,b){const c=await this.call('eth_getCode',[to,tag(b)]);if(!/^0x[0-9a-f]+$/i.test(c)||c==='0x')throw Error('NO_CODE');return c;}
}
export async function mapLimit(items,limit,fn){const out=new Array(items.length);let next=0;await Promise.all(Array.from({length:Math.min(limit,items.length)},async()=>{while(next<items.length){const i=next++;try{out[i]=await fn(items[i],i);}catch(e){out[i]={error:e.message};}}}));return out;}
export async function findStart(rpc,end,target){let lo=Math.max(0,end-60000),hi=end;const first=await rpc.call('eth_getBlockByNumber',[tag(lo),false]);if(Number(BigInt(first.timestamp))>target)throw Error('SEVEN_DAY_SEARCH_BOUND_INSUFFICIENT');while(lo<hi){const m=Math.floor((lo+hi)/2),b=await rpc.call('eth_getBlockByNumber',[tag(m),false]);if(Number(BigInt(b.timestamp))<target)lo=m+1;else hi=m;}return lo;}
export async function scanEvents(rpc,factory,start,end,{chunk=2000}={}){
 const events=[],coverage=[];
 for(let a=start;a<=end;a+=chunk){const b=Math.min(a+chunk-1,end);
  try{const logs=await rpc.call('eth_getLogs',[{address:factory.address,fromBlock:tag(a),toBlock:tag(b),topics:[TOPICS[factory.kind]]}]);
   for(const log of logs){if(!same(log.address,factory.address)||log.removed)throw Error('INVALID_LOG_EMITTER');const e=abi.parseLog(log);events.push({factory:factory.name,kind:factory.kind,token0:e.args.token0,token1:e.args.token1,address:factory.kind==='v2'?e.args.pair:e.args.pool,fee:factory.kind==='v2'?3000:Number(e.args.fee),blockNumber:Number(BigInt(log.blockNumber)),blockHash:log.blockHash,transactionHash:log.transactionHash,logIndex:log.logIndex});}
   coverage.push({from:a,to:b,status:'COMPLETE',logs:logs.length});
  }catch(e){coverage.push({from:a,to:b,status:'FAILED',reason:e.message});}
 }
 return {factory,events,coverage};
}
export async function tokenRisk(rpc,t,b){
 await rpc.code(t,b);const decimals=Number((await rpc.read(t,'decimals',[],b))[0]);if(!Number.isInteger(decimals)||decimals>36)throw Error('UNSUPPORTED_DECIMALS');
 const result={address:t,decimals,transferRisk:'UNKNOWN_NOT_TESTED',feeOnTransfer:'UNKNOWN',rebase:'UNKNOWN',honeypot:'UNKNOWN',proxy:'UNKNOWN',roles:'UNKNOWN',safe:false};
 try{result.owner=(await rpc.read(t,'owner',[],b))[0];}catch{result.ownerGetter='UNAVAILABLE';}
 try{result.paused=(await rpc.read(t,'paused',[],b))[0];}catch{result.pauseGetter='UNAVAILABLE';}
 try{const storage=await rpc.call('eth_getStorageAt',[t,'0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc',tag(b)]);result.eip1967Implementation='0x'+storage.slice(-40);result.proxy=BigInt(storage)!==0n?'EIP1967_SLOT_NONZERO':'STANDARD_SLOT_ZERO_NOT_PROOF';}catch{result.proxy='UNKNOWN_RPC_ERROR';}
 return result;
}
export async function discoverExisting(rpc,other,b){
 const requests=FACTORIES.flatMap(f=>f.kind==='v2'?[{f,fee:3000}]:[100,500,3000,10000].map(fee=>({f,fee})));
 const checks=await mapLimit(requests,2,async({f,fee})=>{
  const addr=(await rpc.read(f.address,f.kind==='v2'?'getPair':'getPool',f.kind==='v2'?[WETH,other]:[WETH,other,fee],b))[0];
  if(BigInt(addr)===0n)return {factory:f.name,fee,status:'MISSING'};
  await rpc.code(addr,b);const t0=(await rpc.read(addr,'token0',[],b))[0],t1=(await rpc.read(addr,'token1',[],b))[0],fac=(await rpc.read(addr,'factory',[],b))[0];
  if(!same(fac,f.address)||!((same(t0,WETH)&&same(t1,other))||(same(t1,WETH)&&same(t0,other))))throw Error('POOL_IDENTITY_MISMATCH');
  if(f.kind==='v3'&&Number((await rpc.read(addr,'fee',[],b))[0])!==fee)throw Error('FEE_MISMATCH');
  return {address:addr,token0:t0,token1:t1,factory:f.address,venue:f.name,kind:f.kind,fee,status:'VERIFIED_IDENTITY'};
 });
 const seen=new Set();return {checks,pools:checks.filter(p=>p.address&&!seen.has(lower(p.address))&&seen.add(lower(p.address)))};
}
export async function state(rpc,p,b){
 if(p.kind==='v2'){const [r0,r1,time]=(await rpc.read(p.address,'getReserves',[],b));if(!r0||!r1)throw Error('EMPTY_RESERVES');return {...p,r0,r1,reserveTimestamp:time,wethDepth:same(p.token0,WETH)?r0:r1};}
 const slot=await rpc.read(p.address,'slot0',[],b),L=(await rpc.read(p.address,'liquidity',[],b))[0];if(!slot[0]||!L)throw Error('ZERO_ACTIVE_LIQUIDITY');return {...p,sqrtPriceX96:slot[0],tick:Number(slot[1]),liquidity:L,unlocked:slot[6],wethDepth:null};
}
export function oriented(p,input){if(p.kind==='v2')return same(input,p.token0)?{ri:p.r0,ro:p.r1}:{ri:p.r1,ro:p.r0};const sq=p.sqrtPriceX96**2n;return same(input,p.token0)?{n:sq,d:1n<<192n}:{n:1n<<192n,d:sq};}
export async function quote(rpc,p,input,amount,b){
 if(p.kind==='v2'){const {ri,ro}=oriented(p,input);return {out:amount*997n*ro/(ri*1000n+amount*997n),ticksCrossed:null,quoteGas:null,quote_verified:false,source:'static V2 integer formula, transfer semantics not tested'};}
 const output=same(input,p.token0)?p.token1:p.token0;
 const q=await rpc.read(QUOTER,'quoteExactInputSingle',[[input,output,amount,p.fee,0]],b);
 return {out:q[0],sqrtAfter:q[1],ticksCrossed:Number(q[2]),quoteGas:q[3],quote_verified:true,source:'canonical QuoterV2 eth_call; not successful token settlement'};
}
export function routeMarginal(a,b){const p=oriented(a,WETH),q=oriented(b,same(a.token0,WETH)?a.token1:a.token0);const n1=a.kind==='v2'?p.ro:p.n,d1=a.kind==='v2'?p.ri:p.d,n2=b.kind==='v2'?q.ro:q.n,d2=b.kind==='v2'?q.ri:q.d;return {n:n1*n2*BigInt(1000000-a.fee)*BigInt(1000000-b.fee),d:d1*d2*1000000n**2n};}
