// READ ONLY: mainnet ETH/USD price check for the first $5 gas trial.
// Never signs, broadcasts, funds, deploys, or enables execution.
export const ETH_USD_FEED = "0x5f4ec3df9cbd43714fe2740f5e3616155c5b8419";
export const MAX_USD_E8 = 500_000_000n;
const WEI_PER_ETH = 10n ** 18n, UINT256_MAX = (1n << 256n) - 1n;
function requireInteger(value, label) {
  if (typeof value !== "bigint" || value <= 0n || value > UINT256_MAX)
    throw new Error(`${label}: missing/out-of-range unsigned integer`);
  return value;
}
function checkedMul(...values) {
  return values.reduce((a,b) => {
    if (b < 0n || a > UINT256_MAX / b) throw new Error("uint256 arithmetic overflow");
    return a * b;
  }, 1n);
}
export function gasUsdE8({gasLimit,maxFeePerGasWei,ethUsdE8}) {
  const g=requireInteger(gasLimit,"gasLimit");
  const f=requireInteger(maxFeePerGasWei,"maxFeePerGasWei");
  const p=requireInteger(ethUsdE8,"ethUsdE8");
  const numerator=checkedMul(g,f,p,110n);
  return (numerator + WEI_PER_ETH*100n-1n)/(WEI_PER_ETH*100n);
}
function hexUint(value, label) {
  if (typeof value !== "string" || !/^0x[0-9a-f]+$/i.test(value))
    throw new Error(`${label}: invalid hex`);
  return BigInt(value);
}
function decodeRoundData(hex) {
  if (typeof hex !== "string" || !/^0x[0-9a-f]{320}$/i.test(hex))
    throw Error("invalid Chainlink round data");
  const words=Array.from({length:5},(_,i)=>BigInt("0x"+hex.slice(2+i*64,2+(i+1)*64)));
  const raw=words[1], answer=raw >= (1n<<255n) ? raw-(1n<<256n) : raw;
  return {roundId:words[0],answer,updatedAt:words[3],answeredInRound:words[4]};
}
export async function inspectGasBudget({rpc,gasLimit,maxFeePerGasWei,maxAgeSeconds=3600n}) {
  if (typeof rpc!=="function") throw Error("verified read-only RPC required");
  if (hexUint(await rpc("eth_chainId",[]),"chainId")!==1n) throw Error("Ethereum mainnet only");
  const [decRaw,roundRaw,block]=await Promise.all([
    rpc("eth_call",[{to:ETH_USD_FEED,data:"0x313ce567"},"latest"]),
    rpc("eth_call",[{to:ETH_USD_FEED,data:"0xfeaf968c"},"latest"]),
    rpc("eth_getBlockByNumber",["latest",false])
  ]);
  if (hexUint(decRaw,"decimals")!==8n) throw Error("Chainlink ETH/USD expected 8 decimals");
  const {roundId,answer,updatedAt,answeredInRound}=decodeRoundData(roundRaw);
  const blockTs=hexUint(block?.timestamp,"block timestamp");
  if (roundId===0n || answeredInRound<roundId || answer<=0n || updatedAt===0n
      || updatedAt>blockTs || blockTs-updatedAt>maxAgeSeconds)
    throw Error("Chainlink ETH/USD invalid or stale round");
  const usd=gasUsdE8({gasLimit,maxFeePerGasWei,ethUsdE8:answer});
  return {chainId:"1",oracle:ETH_USD_FEED,gasUsdE8:usd.toString(),
    allowedByFiveUsd:usd<=MAX_USD_E8,gasLimit:gasLimit.toString(),
    maxFeePerGasWei:maxFeePerGasWei.toString(),
    ethUsdE8:answer.toString(),quoteAgeSeconds:(blockTs-updatedAt).toString()};
}
async function readRpc(url,method,params) {
  let res;
  try {res=await fetch(url,{method:"POST",headers:{"content-type":"application/json"},
    body:JSON.stringify({jsonrpc:"2.0",id:1,method,params}),
    signal:AbortSignal.timeout(5000)});}
  catch {throw Error("RPC unavailable (credentials hidden)");}
  if (!res.ok) throw Error(`RPC HTTP ${res.status}`);
  let data;try {data=await res.json();} catch {throw Error("RPC invalid JSON")}
  if (data?.error || data?.result===undefined)
    throw Error(`RPC ${method} rejected or missing response`);
  return data.result;
}
if (import.meta.url===`file://${process.argv[1]}`) {
  try {
    const url=process.env.HTTPS_URL;
    if (!url || !/^https:\/\//i.test(url)) throw Error("private HTTPS_URL required");
    const gasLimit=BigInt(process.env.SIM_V2_GAS_UNITS||"1000000");
    // Explicitly require a verified transaction maximum EIP-1559 cap.
    const maxFeePerGasWei=BigInt(process.env.SIM_V2_MAX_FEE_PER_GAS_WEI||"0");
    const output=await inspectGasBudget({rpc:(m,p)=>readRpc(url,m,p),gasLimit,maxFeePerGasWei});
    console.log(JSON.stringify(output,null,2));
    if (!output.allowedByFiveUsd) process.exitCode=2;
  } catch(e) {
    console.error("FAIL-CLOSED read-only gas check:",e.message);
    process.exitCode=2;
  }
}
