#!/usr/bin/env node
// Base 8453, two publicly identified 5-bp candidate pools, read-only RPC validation.
// No keys, private data, transactions, signing, continuous loops or automatic retries.
import { pathToFileURL } from "node:url";

export const EXPECTED = Object.freeze({
  chain: "0x2105",
  weth: "0x4200000000000000000000000000000000000006",
  usdc: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".toLowerCase(),
  pools: [
    {venue:"Uniswap V3", address:"0xd0b53d9277642d899df5c87a3966a349a798f224"},
    {venue:"SushiSwap V3", address:"0x57713f7716e0b0f65ec116912f834e49805480d2"}
  ],
  fee:500, // 0.05% expressed in V3 hundredths-of-a-bip units
});
export function decodeWord(value) {
  if (typeof value !== "string" || !/^0x[0-9a-fA-F]{64}$/.test(value)) throw new Error("invalid 32-byte RPC word");
  return value.slice(2);
}
export function decodeAddress(value) { return "0x" + decodeWord(value).slice(24).toLowerCase(); }
export function decodeUint(value) { return BigInt("0x" + decodeWord(value)).toString(); }
export function validateMetadata(item) {
  if (!item.code || item.code === "0x") throw new Error("pool has no bytecode");
  const tokens=[item.token0.toLowerCase(),item.token1.toLowerCase()].sort();
  const expected=[EXPECTED.weth.toLowerCase(),EXPECTED.usdc].sort();
  if (tokens.join(",")!==expected.join(",")) throw new Error("wrong tokens");
  if (item.token0.toLowerCase()===item.token1.toLowerCase()) throw new Error("identical tokens");
  if (item.fee !== String(EXPECTED.fee)) throw new Error("wrong pool fee");
  if (BigInt(item.liquidity)===0n) throw new Error("zero active liquidity");
  if (BigInt(item.sqrtPriceX96)===0n) throw new Error("uninitialized price");
}
export async function inspect(fetcher = fetch, endpoint = process.env.BASE_RPC_URL || "https://mainnet.base.org") {
  const u=new URL(endpoint);
  if(u.protocol!=="https:" || u.username || u.password || u.search || u.hash) throw new Error("HTTPS URL without embedded query credentials required");
  let calls=0;
  async function rpc(method,params) {
    calls++;
    if(calls>18) throw new Error("RPC read budget exceeded");
    const response=await fetcher(endpoint,{
      method:"POST",headers:{"content-type":"application/json"},
      body:JSON.stringify({jsonrpc:"2.0",id:calls,method,params}),
      signal:AbortSignal.timeout(8000),
    });
    if(!response.ok) throw new Error(method+" HTTP "+response.status);
    const body=await response.json();
    if(body?.error || body?.result==null) throw new Error(method+" RPC error");
    return body.result;
  }
  const chain=await rpc("eth_chainId",[]);
  if(chain.toLowerCase()!==EXPECTED.chain) throw new Error("not Base mainnet");
  const block=await rpc("eth_blockNumber",[]);
  if(!/^0x[0-9a-f]+$/i.test(block)) throw new Error("invalid block tag");
  // Each read pinned to the SAME block, avoiding mixed-block comparisons.
  const selectors={token0:"0x0dfe1681",token1:"0xd21220a7",fee:"0xddca3f43",
    liquidity:"0x1a686502",slot0:"0x3850c7bd",factory:"0xc45a0155"};
  const results=[];
  for(const p of EXPECTED.pools) {
    const code=await rpc("eth_getCode",[p.address,block]);
    const words={};
    for(const [key,selector] of Object.entries(selectors)) words[key]=await rpc("eth_call",[{to:p.address,data:selector},block]);
    const slot=words.slot0;
    if(typeof slot!=="string" || !/^0x[0-9a-f]{64,}$/i.test(slot)) throw new Error("invalid slot0 result");
    const item={venue:p.venue,address:p.address,code,
      token0:decodeAddress(words.token0),token1:decodeAddress(words.token1),
      fee:decodeUint(words.fee),liquidity:decodeUint(words.liquidity),
      sqrtPriceX96:BigInt("0x"+slot.slice(2,66)).toString(),
      factory:decodeAddress(words.factory)};
    validateMetadata(item);
    // Do not output full code, raw RPC errors or endpoint URL.
    results.push({venue:item.venue,address:item.address,token0:item.token0,token1:item.token1,
      fee:item.fee,liquidity:item.liquidity,sqrtPriceX96:item.sqrtPriceX96,factory:item.factory,
      verifiedTokenAndFee:true,factoryCanonicalVerified:false});
  }
  return {chainId:8453,block,pools:results,structuralComparisonReady:true,
    executableQuoteVerified:false,netProfitVerified:false,txSent:0,rpcCalls:calls};
}
if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  inspect().then(x=>console.log(JSON.stringify(x,null,2))).catch(e=>{
    console.error(JSON.stringify({status:"UNVERIFIED",reason:e.message,txSent:0}));
    process.exitCode=1;
  });
}
