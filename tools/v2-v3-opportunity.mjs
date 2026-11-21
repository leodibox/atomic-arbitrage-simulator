#!/usr/bin/env node
// One-shot REAL Ethereum quote comparison, read-only. No dependencies.
// Intentionally separate from production engine and NO private keys.
// Node 18+ required. Usage:
//   node tools/v2-v3-opportunity.mjs
//   HTTPS_URL=https://... node tools/v2-v3-opportunity.mjs
const RPC = process.env.HTTPS_URL || "https://ethereum-rpc.publicnode.com";
const WETH = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
const USDC = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const UNI_V2 = "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc";
const SUSHI_V2 = "0x397FF1542f962076d0BFE58eA045FfA2d347ACa0";
const UNI_FACTORY = "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f";
const SUSHI_FACTORY = "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac";
const QUOTER_V2 = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";
const ORACLE = "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419";
const GAS_UNITS = 1_000_000n;
const TIP = 2_000_000_000n;
const AAVE_PREMIUM_BPS = 5n; // Verified at recent fork; conservative screen, must recheck before execution
const SLIPPAGE_BPS = 10n;
const AMOUNTS = [10n**16n, 5n*10n**16n, 10n**17n, 25n*10n**16n];
const FEES = [500n, 3000n];
const EXIT = (message) => { console.error("FAIL-CLOSED:", message); process.exitCode = 1; };
const pad = (n) => BigInt(n).toString(16).padStart(64,"0");
const addr = (a) => pad(BigInt(a));
const norm = (a) => "0x"+a.slice(-40).toLowerCase();
function word(raw,i=0) {
  if (typeof raw !== "string" || !/^0x[0-9a-fA-F]*$/.test(raw) || raw.length < 2+(i+1)*64)
    throw Error("Bad ABI response");
  return BigInt("0x"+raw.slice(2+i*64,2+(i+1)*64));
}
let nextId=1, attempts=0, rpcErrors=0;
async function rpc(method,params) {
  attempts++;
  const response=await fetch(RPC,{
    method:"POST",headers:{"content-type":"application/json"},
    body:JSON.stringify({jsonrpc:"2.0",id:nextId++,method,params}),
    signal:AbortSignal.timeout(12000)
  });
  if(!response.ok) throw Error("RPC HTTP "+response.status);
  const json=await response.json();
  if(json.error) {rpcErrors++;throw Error("RPC "+(json.error.message||json.error.code));}
  if(json.result===undefined) throw Error("RPC missing result");
  return json.result;
}
async function call(to,data,block){return rpc("eth_call",[{to,data},block]);}
async function pair(address,expectedFactory,block){
  const [factory,t0,t1,r] = await Promise.all([
    call(address,"0xc45a0155",block),call(address,"0x0dfe1681",block),
    call(address,"0xd21220a7",block),call(address,"0x0902f1ac",block)
  ]);
  if(norm(factory)!==expectedFactory.toLowerCase() ||
     ![WETH.toLowerCase(),USDC.toLowerCase()].includes(norm(t0)) ||
     ![WETH.toLowerCase(),USDC.toLowerCase()].includes(norm(t1)) ||
     norm(t0)===norm(t1)) throw Error("V2 identity mismatch "+address);
  const x=word(r,0),y=word(r,1);
  const weth0=norm(t0)===WETH.toLowerCase();
  return {address,weth:weth0?x:y,usdc:weth0?y:x};
}
function v2Quote(p,amount,inputWeth){
  const i=inputWeth?p.weth:p.usdc, o=inputWeth?p.usdc:p.weth;
  if(i===0n||o===0n||amount===0n) return null;
  return amount*997n*o/(i*1000n+amount*997n);
}
async function v3Quote(tokenIn,tokenOut,amount,fee,block){
  // Official QuoterV2 quoteExactInputSingle((address,address,uint256,uint24,uint160)).
  // Read-only eth_call, never a transaction.
  const data="0xc6a5026a"+addr(tokenIn)+addr(tokenOut)+pad(amount)+pad(fee)+pad(0);
  const raw=await call(QUOTER_V2,data,block);
  return word(raw);
}
const fmt=(x,scale=18n)=>Number(x)/10**Number(scale);
async function main(){
  if(!/^https:\/\//.test(RPC)) throw Error("HTTPS_URL must be an https RPC endpoint");
  const chain=BigInt(await rpc("eth_chainId",[]));
  if(chain!==1n) throw Error("Only Ethereum mainnet (chain id 1) is supported");
  const latest=await rpc("eth_getBlockByNumber",["latest",false]);
  if(!latest || !latest.baseFeePerGas || !latest.timestamp) throw Error("Missing latest EIP1559 block");
  const block=latest.number, at=BigInt(latest.timestamp);
  // Quote USD with Chainlink (must be fresh); no fallback prices allowed.
  const [oracleDecimals,round]=await Promise.all([
    call(ORACLE,"0x313ce567",block),
    call(ORACLE,"0xfeaf968c",block)
  ]);
  const decimals=word(oracleDecimals);
  const roundId=word(round,0), price=word(round,1), updated=word(round,3), answered=word(round,4);
  if(decimals!==8n||roundId===0n||answered<roundId||price===0n||
     updated>at||at-updated>3600n) throw Error("Invalid/stale Chainlink ETH/USD");
  const gasWei=GAS_UNITS*(2n*BigInt(latest.baseFeePerGas)+TIP);
  const gasUsdE8=(gasWei*price*110n + 100n*10n**18n-1n)/(100n*10n**18n);
  const pools=await Promise.all([
    pair(UNI_V2,UNI_FACTORY,block),
    pair(SUSHI_V2,SUSHI_FACTORY,block)
  ]);
  const results=[],errors=[];
  for(const amount of AMOUNTS){
    for(const fee of FEES){
      for(const p of pools){
        // WETH -> USDC on V2; return USDC -> WETH on V3.
        const usdc=v2Quote(p,amount,true);
        try {
          const back=await v3Quote(USDC,WETH,usdc,fee,block);
          results.push(evaluate(amount,back,p.address,"V2->V3",fee,gasWei));
        }catch(e){ errors.push("V2->V3 "+p.address+" fee="+fee+": "+e.message); }
        // WETH -> USDC on V3; return USDC -> WETH on V2.
        try {
          const usdcV3=await v3Quote(WETH,USDC,amount,fee,block);
          const back=v2Quote(p,usdcV3,false);
          if(back===null)throw Error("No V2 reserves");
          results.push(evaluate(amount,back,p.address,"V3->V2",fee,gasWei));
        }catch(e){ errors.push("V3->V2 "+p.address+" fee="+fee+": "+e.message); }
      }
    }
  }
  const sorted=results.sort((a,b)=>a.netWei>b.netWei?-1:a.netWei<b.netWei?1:0);
  console.log(JSON.stringify({
    mode:"READ_ONLY_NO_TRADES",network:"ethereum",block:Number(BigInt(block)),
    gasModel:{units:Number(GAS_UNITS),maxFeeWei:(gasWei/GAS_UNITS).toString(),
      conservativeUsd:Number(gasUsdE8)/1e8,hardCapUsd:5,withinHardCap:gasUsdE8<=500000000n,
      ethUsd:Number(price)/1e8,oracleAgeSec:Number(at-updated)},
    markets:"WETH-USDC: real UniswapV2 and SushiV2 vs UniswapV3 QuoterV2 fees 500/3000",
    quotesAttempted:AMOUNTS.length*FEES.length*pools.length*2,
    quotesSucceeded:results.length,quoteErrors:errors.length,
    positiveBeforeGas:results.filter(x=>x.afterPremiumWei>0n).length,
    positiveAfterCosts:results.filter(x=>x.netWei>0n).length,
    actionableForNextFork:results.filter(x=>x.netWei>0n&&gasUsdE8<=500000000n).length,
    top:sorted.slice(0,5).map(x=>({
      route:x.route,v2Pair:x.v2Pair,feeV3:x.feeV3,
      loanWeth:fmt(x.loanWei),quotedReturnWeth:fmt(x.outWei),
      afterPremiumWeth:fmt(x.afterPremiumWei),
      conservativeNetWeth:fmt(x.netWei)
    })),errors:errors.slice(0,4),
    disclaimer:"Quote is not execution. No keys, no send, no balance change. Aave premium assumed 5bps from prior verified fork; real fee, exact contract compatibility and gas require fresh preflight."
  },null,2));
  if(errors.length) EXIT("Some V3 quotes unavailable: do not interpret as complete market coverage");
}
function evaluate(loanWei,outWei,v2Pair,route,feeV3,gasWei){
  const premium=(loanWei*AAVE_PREMIUM_BPS+9999n)/10000n;
  const afterPremiumWei=outWei-loanWei-premium;
  const conservativeOut=outWei*(10000n-SLIPPAGE_BPS)/10000n;
  const netWei=conservativeOut-loanWei-premium-gasWei;
  return {loanWei,outWei,v2Pair,route,feeV3:Number(feeV3),afterPremiumWei,netWei};
}
main().catch(e=>EXIT(e.message));
