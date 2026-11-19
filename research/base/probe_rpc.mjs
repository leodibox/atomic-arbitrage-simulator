#!/usr/bin/env node
// Bounded, read-only Base RPC capability probe. No keys, wallet, signing, retries, or historical backfill.
// Usage: node research/base/probe_rpc.mjs
const endpoint = process.env.BASE_RPC_URL || "https://mainnet.base.org";
const parsed = new URL(endpoint);
if (parsed.protocol !== "https:") throw new Error("BASE_RPC_URL must use HTTPS");
const methods = [
  ["eth_chainId", []],
  ["eth_blockNumber", []],
  ["eth_gasPrice", []],
  ["eth_feeHistory", ["0x1", "latest", []]],
];
let calls = 0;
async function rpc(method, params) {
  calls++;
  const response = await fetch(endpoint, {
    method: "POST",
    headers: {"content-type": "application/json"},
    body: JSON.stringify({jsonrpc:"2.0", id:calls, method, params}),
    signal: AbortSignal.timeout(8000),
  });
  let payload;
  try { payload = await response.json(); } catch { payload = null; }
  return {status: response.status, result: payload?.result, errorCode:payload?.error?.code ?? null};
}
for (const [method,params] of methods) {
  try {
    const response = await rpc(method, params);
    const value = method === "eth_feeHistory"
      ? {baseFeePerGas: response.result?.baseFeePerGas?.slice(-2) ?? null}
      : response.result ?? null;
    console.log(JSON.stringify({method, httpStatus:response.status, value, rpcErrorCode:response.errorCode}));
    if (method==="eth_chainId" && value!=="0x2105") {
      console.error("STOP: endpoint does not identify as Base mainnet chain 8453");
      process.exitCode=1;
      break;
    }
    if (response.status!==200 || response.errorCode!==null) {
      console.error("STOP: RPC denied or failed; no retries");
      process.exitCode=1;
      break;
    }
  } catch (err) {
    console.error(JSON.stringify({method, error:err?.name ?? "Error", message:"RPC connection/timeout failure"}));
    process.exitCode=1;
    break;
  }
}
console.log(JSON.stringify({calls, readonly:true, transactionsSent:0, profitVerified:false}));
