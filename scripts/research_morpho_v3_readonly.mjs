#!/usr/bin/env node
// Read-only feasibility: Morpho Blue 0-fee flash funding and Uniswap v3 low-fee pools.
// No signature, no eth_sendTransaction, no eth_sendRawTransaction, no approvals.
// All returned routes are INELIGIBLE_FOR_EXECUTION.
import { pathToFileURL } from "node:url";

export const ADDRESS = Object.freeze({
  morpho: "0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb",
  aave: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
  factory: "0x1F98431c8aD98523631AE4a59f267346ea31F984",
  weth: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
  usdc: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
});
const SELECTOR = Object.freeze({
  balanceOf: "0x70a08231", getPool: "0x1698ee82",
  fee: "0xddca3f43", factory: "0xc45a0155",
  token0: "0x0dfe1681", token1: "0xd21220a7",
  liquidity: "0x1a686502", premium: "0x074b2e43"
});
const toWord = a => a.slice(2).toLowerCase().padStart(64, "0");
const toFee = x => BigInt(x).toString(16).padStart(64, "0");
const equals = (a,b) => a.toLowerCase() === b.toLowerCase();
const decode = value => {
  if(typeof value!=="string"||!/^0x[0-9a-f]+$/i.test(value)||value.length<66)
    throw Error("Invalid ABI word");
  return BigInt("0x"+value.slice(2,66));
};
const addr = v => "0x"+decode(v).toString(16).padStart(40,"0");
const EMPTY = "0x"+"0".repeat(40);

export function feeScenarios({v3FeeBps, aaveFeeBps}) {
  if(!Number.isInteger(v3FeeBps)||v3FeeBps<=0||aaveFeeBps<0n)
    throw Error("Invalid fee quote");
  // Illustrative fee-only lower bounds, not executable or profitable quotes.
  return {
    v3ThenV2WithMorphoBps: BigInt(v3FeeBps+30),
    v3ThenV2WithAaveBps: BigInt(v3FeeBps+30)+aaveFeeBps,
    v3ThenV3WithMorphoBps: BigInt(v3FeeBps*2),
    v3ThenV3WithAaveBps: BigInt(v3FeeBps*2)+aaveFeeBps,
    approvedForExecution: false
  };
}

export async function collectReadOnlyEvidence(rpc, {tag="finalized"}={}) {
  if(!/^(latest|safe|finalized|0x[0-9a-f]+)$/i.test(tag))throw Error("Invalid block tag");
  if(BigInt(await rpc("eth_chainId",[]))!==1n)throw Error("Wrong chain");
  const block = await rpc("eth_getBlockByNumber",[tag,false]);
  if(!block?.number||!block?.hash||!block?.timestamp)throw Error("Missing block identity");
  const number=BigInt(block.number);
  if(tag.startsWith("0x")&&number!==BigInt(tag))throw Error("Mismatched block");
  const pinned="0x"+number.toString(16);
  const call=(to,data)=>rpc("eth_call",[{to,data},pinned]);
  const code=to=>rpc("eth_getCode",[to,pinned]);
  const [morphoCode,aaveCode,factoryCode,wethCode,usdcCode]=await Promise.all([
    code(ADDRESS.morpho),code(ADDRESS.aave),code(ADDRESS.factory),code(ADDRESS.weth),code(ADDRESS.usdc)
  ]);
  if([morphoCode,aaveCode,factoryCode,wethCode,usdcCode]
    .some(x=>typeof x!=="string"||x==="0x"||x==="0x0"))
    throw Error("Canonical contract bytecode absent");
  const [aavePremium, wethCash, usdcCash] = await Promise.all([
    call(ADDRESS.aave,SELECTOR.premium),
    call(ADDRESS.weth,SELECTOR.balanceOf+toWord(ADDRESS.morpho)),
    call(ADDRESS.usdc,SELECTOR.balanceOf+toWord(ADDRESS.morpho))
  ]);
  const aaveBps=decode(aavePremium);
  if(aaveBps>1000n)throw Error("Aave premium unreasonable");
  const morphoBalances={WETH:decode(wethCash),USDC:decode(usdcCash)};
  const pools=[];
  for(const fee of [100,500]){
    const pairResp=await call(ADDRESS.factory,SELECTOR.getPool+
      toWord(ADDRESS.usdc)+toWord(ADDRESS.weth)+toFee(fee));
    const poolAddress=addr(pairResp);
    if(equals(poolAddress,EMPTY)){
      pools.push({feeHundredthBips:fee,feeBps:fee/100,status:"POOL_MISSING",approvedForExecution:false});
      continue;
    }
    if((await code(poolAddress))==="0x")throw Error("V3 pool bytecode absent");
    const [factory,token0,token1,feeData,liquidity]=await Promise.all([
      call(poolAddress,SELECTOR.factory),call(poolAddress,SELECTOR.token0),
      call(poolAddress,SELECTOR.token1),call(poolAddress,SELECTOR.fee),
      call(poolAddress,SELECTOR.liquidity)
    ]);
    const t0=addr(token0),t1=addr(token1),reportedFee=decode(feeData);
    if(!equals(addr(factory),ADDRESS.factory)||reportedFee!==BigInt(fee)||
      !((equals(t0,ADDRESS.weth)&&equals(t1,ADDRESS.usdc))||
         (equals(t1,ADDRESS.weth)&&equals(t0,ADDRESS.usdc))))
      throw Error("Uniswap V3 factory/pool identity mismatch");
    const inRange=decode(liquidity);
    pools.push({address:poolAddress,feeHundredthBips:fee,feeBps:fee/100,
      liquidityRaw:inRange, status:inRange>0n?"IN_RANGE_LIQUIDITY_PRESENT":"ZERO_IN_RANGE_LIQUIDITY",
      feeOnlyScenarios:feeScenarios({v3FeeBps:fee/100,aaveFeeBps:aaveBps}),
      approvedForExecution:false});
  }
  return {chainId:1,blockNumber:number,blockHash:block.hash,blockTimestamp:BigInt(block.timestamp),
    providers:{morpho:{address:ADDRESS.morpho,
      documentedFlashFeeBps:0,feeWasVerifiedFromContract:false,
      cashBalancesRaw:morphoBalances,liquidityAvailabilityVerified:false,
      callbackIntegrationVerified:false,approvedForExecution:false},
      aave:{address:ADDRESS.aave,onchainPremiumBps:aaveBps}},
    uniswapV3Pools:pools,
    assumptions:{morphoFeeSource:"Morpho Blue official contract docs, NOT an onchain fee getter",
      cashBalanceIsNotFlashLoanExecutionProof:true,
      v3PriceQuoteVerified:false,tickCrossingQuoteVerified:false,
      actualGasEstimated:false,netProfitVerified:false,
      transactionSubmissionEnabled:false,approvedForExecution:false}};
}
export async function rpcHttps(url,method,params=[],request=fetch){
  if(typeof url!=="string"||!url.startsWith("https://"))throw Error("HTTPS_URL required privately");
  let response;
  try{response=await request(url,{method:"POST",headers:{"content-type":"application/json"},
    body:JSON.stringify({jsonrpc:"2.0",id:1,method,params})});}
  catch{throw Error("RPC connection failed (credentials suppressed)");}
  if(!response.ok)throw Error("RPC rejected with HTTP "+response.status);
  let json;try{json=await response.json();}catch{throw Error("Malformed RPC JSON");}
  if(json?.error||json?.result===undefined||json?.result===null)
    throw Error("RPC method failed: "+method);
  return json.result;
}
if(process.argv[1]&&pathToFileURL(process.argv[1]).href===import.meta.url){
  try{
    const snap=await collectReadOnlyEvidence(
      (method,params)=>rpcHttps(process.env.HTTPS_URL,method,params),
      {tag:process.env.V3_BLOCK_TAG||"finalized"});
    console.log(JSON.stringify(snap,(_,v)=>typeof v==="bigint"?v.toString():v,2));
  }catch(e){console.error("Read-only Morpho/V3 feasibility failed:",e.message);process.exitCode=2;}
}
