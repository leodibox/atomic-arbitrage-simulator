#!/usr/bin/env node
// Base one-shot economic opportunity discovery. REAL Base pool quotes; NO SEND.
// Requires Node >=18 and existing Foundry cast (only to compute ABI selectors).
import { execFileSync } from "node:child_process";

const RPC=process.env.BASE_RPC_URL||"https://mainnet.base.org";
const WETH="0x4200000000000000000000000000000000000006";
const USDC="0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";
const AERO_FACTORY="0x420DD381b31aEf6683db6B902084cB0FFECe40Da";
const QUOTER="0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a";
const FEED="0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70";
const AMOUNTS=[5n,10n,25n,50n].map(x=>x*10n**16n); // 0.05/0.10/0.25/0.50 WETH
const FEES=[500n,3000n], STABLE=[false,true];
const FLASH_BPS=5n; // Previously measured on L1; Base Aave fee is NOT verified.
const SLIPPAGE_BPS=10n, GAS_UNITS=1_000_000n;
const MIN_WETH=5n*10n**18n;
const pad=x=>BigInt(x).toString(16).padStart(64,"0");
const encAddr=x=>pad(BigInt(x));
const addressOf=x=>"0x"+x.slice(-40).toLowerCase();
const eth=x=>Number(x)/1e18;
function sig(signature){
  return execFileSync("cast",["sig",signature],{encoding:"utf8",timeout:5000}).trim();
}
const SIG={
  getPool:sig("getPool(address,address,bool)"),
  amountOut:sig("getAmountOut(uint256,address)"),
  quote:sig("quoteExactInputSingle((address,address,uint256,uint24,uint160))")
};
function word(raw,i=0){
  if(typeof raw!=="string"||!/^0x[0-9a-fA-F]+$/.test(raw)||
    raw.length<2+64*(i+1))throw Error("invalid ABI response");
  return BigInt("0x"+raw.slice(2+i*64,2+(i+1)*64));
}
let sequence=1;
async function rpc(method,params){
  const res=await fetch(RPC,{method:"POST",headers:{"content-type":"application/json"},
    body:JSON.stringify({jsonrpc:"2.0",id:sequence++,method,params}),
    signal:AbortSignal.timeout(16000)});
  if(!res.ok)throw Error("HTTP "+res.status);
  const j=await res.json();
  if(j.error)throw Error(j.error.message||JSON.stringify(j.error));
  if(j.result===undefined)throw Error("missing result");
  return j.result;
}
const call=(to,data,block)=>rpc("eth_call",[{to,data},block]);
async function loadPool(stable,block){
  const raw=await call(AERO_FACTORY,SIG.getPool+encAddr(WETH)+encAddr(USDC)+pad(stable?1:0),block);
  const address=addressOf(raw);
  if(address==="0x0000000000000000000000000000000000000000")return null;
  const [factory,a,b,rv,flag]=await Promise.all([
    call(address,"0xc45a0155",block),call(address,"0x0dfe1681",block),
    call(address,"0xd21220a7",block),call(address,"0x0902f1ac",block),
    call(address,sig("stable()"),block)
  ]);
  if(addressOf(factory)!==AERO_FACTORY.toLowerCase()||
     ![addressOf(a),addressOf(b)].includes(WETH.toLowerCase())||
     ![addressOf(a),addressOf(b)].includes(USDC.toLowerCase())||
     addressOf(a)===addressOf(b)||word(flag)!==BigInt(stable))
    throw Error("Aerodrome pool identity mismatch "+address);
  const reserves=[word(rv,0),word(rv,1)];
  const wethReserve=addressOf(a)===WETH.toLowerCase()?reserves[0]:reserves[1];
  return {address,stable,wethReserve,eligible:wethReserve>=MIN_WETH};
}
async function aeroQuote(pool,amount,inputToken,block){
  return word(await call(pool.address,SIG.amountOut+pad(amount)+encAddr(inputToken),block));
}
async function uniQuote(tokenIn,tokenOut,amount,fee,block){
  return word(await call(QUOTER,SIG.quote+encAddr(tokenIn)+encAddr(tokenOut)+pad(amount)+pad(fee)+pad(0),block));
}
const summary=(r)=>({pool:r.pool,stable:r.stable,feeV3:r.feeV3,
  route:r.route,loanWeth:eth(r.input),returnWeth:eth(r.output),
  preGasAfterAssumedPremiumWeth:eth(r.afterPremium),
  indicativeAfterReservedCostsWeth:eth(r.afterCosts)});
async function main(){
  if(!RPC.startsWith("https://"))throw Error("HTTPS Base RPC required");
  if(BigInt(await rpc("eth_chainId",[]))!==8453n)throw Error("Not Base mainnet chain 8453");
  const head=await rpc("eth_getBlockByNumber",["latest",false]);
  if(!head?.number||!head?.timestamp)throw Error("missing Base block");
  const block=head.number, now=BigInt(head.timestamp);
  const [feedDecimals,feedData,gasPrice]=await Promise.all([
    call(FEED,"0x313ce567",block),call(FEED,"0xfeaf968c",block),rpc("eth_gasPrice",[])
  ]);
  const rid=word(feedData,0),price=word(feedData,1),updated=word(feedData,3),answered=word(feedData,4);
  if(word(feedDecimals)!==8n||rid===0n||answered<rid||price===0n||
    price>=1n<<255n||updated>now||now-updated>3600n)
    throw Error("Chainlink Base ETH/USD unavailable or stale");
  // Indicative L2 execution-only upper bound + $0.50 L1-data reserve.
  // Not exact; actual Base fees, sequencer priority and protocol flash fee
  // remain unverified. No quote is called executable.
  const l2Wei=GAS_UNITS*BigInt(gasPrice)*2n;
  const usdReservedE8=(l2Wei*price/10n**18n)+50_000_000n;
  const reserveWei=(usdReservedE8*10n**18n+price-1n)/price;
  const pools=[],issues=[],results=[];
  for(const stable of STABLE){
    try{
      const pool=await loadPool(stable,block);
      pools.push(pool?{stable,address:pool.address,wethReserve:eth(pool.wethReserve),
        eligible:pool.eligible}:{stable,address:null,eligible:false});
      if(!pool||!pool.eligible)continue;
      for(const amount of AMOUNTS)for(const fee of FEES){
        try{
          const outToken=await aeroQuote(pool,amount,WETH,block);
          const outWeth=await uniQuote(USDC,WETH,outToken,fee,block);
          results.push(score(pool,amount,outWeth,fee,"Aerodrome->UniswapV3",reserveWei));
        }catch(e){issues.push({stable,fee:Number(fee),direction:"Aero->V3",error:String(e.message)});}
        try{
          const outToken=await uniQuote(WETH,USDC,amount,fee,block);
          const outWeth=await aeroQuote(pool,outToken,USDC,block);
          results.push(score(pool,amount,outWeth,fee,"UniswapV3->Aerodrome",reserveWei));
        }catch(e){issues.push({stable,fee:Number(fee),direction:"V3->Aero",error:String(e.message)});}
      }
    }catch(e){issues.push({stable,poolQueryError:String(e.message)});}
  }
  results.sort((a,b)=>a.afterCosts>b.afterCosts?-1:a.afterCosts<b.afterCosts?1:0);
  const grossPositive=results.filter(x=>x.afterPremium>0n);
  const indicative=results.filter(x=>x.afterCosts>0n&&usdReservedE8<=500_000_000n);
  console.log(JSON.stringify({
    mode:"BASE_READ_ONLY_NO_TRADES",block:parseInt(block,16),chainId:8453,
    market:"WETH/USDC Aerodrome volatile/stable vs Uniswap V3 0.05%/0.3%",
    pools,quotesAttempted:results.length+issues.filter(x=>x.direction).length,
    quotesSucceeded:results.length,quoteErrors:issues.length,
    positiveAfterAssumedFlashBeforeGas:grossPositive.length,
    indicativePositiveAfterReservedCosts:indicative.length,
    gas:{l2GasPriceWei:BigInt(gasPrice).toString(),estimatedL2GasWei:l2Wei.toString(),
      provisionalL1ReserveUsd:0.50,indicativeTotalReservedUsd:Number(usdReservedE8)/1e8,
      senderHardCapUsd:5,withinFiveUsdIndicative:usdReservedE8<=500_000_000n,
      ethUsd:Number(price)/1e8},
    top:results.slice(0,5).map(summary),issues:issues.slice(0,8),
    decision:issues.length?"INCONCLUSIVE_MISSING_QUOTES":
      indicative.length?"CANDIDATE_ONLY_REQUIRES_EXACT_BASE_EXECUTION_AND_FEE_VALIDATION":
      "NO_GO_FOR_THIS_BLOCK",
    disclaimer:"Base strategy is NOT deployed. Flash premium assumed from Ethereum, not verified on Base. Fee model reserves $0.50 for L1 data but is NOT an exact transaction fee. No send, keys, or funds; $5 limit on existing Ethereum sender is NOT transferable to Base."
  },null,2));
  if(issues.length)process.exitCode=2;
}
function score(pool,input,output,fee,route,feeReserveWei){
  const premium=(input*FLASH_BPS+9999n)/10000n;
  const afterPremium=output-input-premium;
  const afterCosts=output*(10000n-SLIPPAGE_BPS)/10000n-input-premium-feeReserveWei;
  return {pool:pool.address,stable:pool.stable,input,output,
    feeV3:Number(fee),route,afterPremium,afterCosts};
}
main().catch(e=>{console.error("FAIL_CLOSED",e.message);process.exitCode=1});
