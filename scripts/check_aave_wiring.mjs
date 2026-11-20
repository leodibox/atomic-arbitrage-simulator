#!/usr/bin/env node
// Read-only Ethereum mainnet Aave / payout wiring attestation.
// Does NOT sign, deploy, approve tokens, submit transactions or print RPC keys.
// Requires Node 18+ and an existing HTTPS_URL (Alchemy or compatible RPC).
import { createHash } from "node:crypto";
import { pathToFileURL } from "node:url";

export const AAVE_V3_ETHEREUM_POOL = "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2";
const SELECTORS = {
  premium: "0x074b2e43",   // FLASHLOAN_PREMIUM_TOTAL()
  owner: "0x8da5cb5b",     // owner()
  recipient: "0x7e2bad1e", // profitRecipient()
  pool: "0x16f0115b",      // pool()
  paused: "0x5c975abb",    // paused()
  autoPayout: "0x7764db39" // autoPayout()
};

function normalizedAddress(value, name) {
  if (typeof value !== "string" || !/^0x[0-9a-fA-F]{40}$/.test(value) ||
      /^0x0{40}$/i.test(value)) {
    throw new Error(`${name} must be a nonzero Ethereum address`);
  }
  return value.toLowerCase();
}

function decodeAddress(word, name) {
  if (typeof word !== "string" || !/^0x[0-9a-fA-F]{64}$/.test(word))
    throw new Error(`${name} getter missing or invalid: executor may be unverified/legacy`);
  return normalizedAddress("0x" + word.slice(-40), name);
}

function decodeUInt(word, name) {
  if (typeof word !== "string" || !/^0x[0-9a-fA-F]{1,64}$/.test(word))
    throw new Error(`${name} getter returned no value or malformed data`);
  return BigInt(word);
}

export async function rpcPost(url, method, params = [], fetchFn = fetch) {
  // Never include the URL or JSON-RPC response in the thrown error; URLs may
  // embed private API keys, and provider errors can echo sensitive strings.
  let response;
  try {
    response = await fetchFn(url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params })
    });
  } catch {
    throw new Error("RPC connection failed (no credentials displayed)");
  }
  if (!response.ok) throw new Error(`RPC HTTP ${response.status}`);
  let body;
  try { body = await response.json(); } catch { throw new Error("RPC returned invalid JSON"); }
  if (body?.error) throw new Error(`RPC rejected ${method}, code ${body.error.code ?? "unknown"}`);
  if (!("result" in (body ?? {}))) throw new Error(`RPC returned no result for ${method}`);
  return body.result;
}

export async function inspectAaveWiring({
  rpcUrl, executorAddress, expectedRecipient, expectedOwner,
  expectedRuntimeCodeSha256
}, rpc = rpcPost) {
  if (!rpcUrl || !/^https:\/\//i.test(rpcUrl))
    throw new Error("HTTPS_URL must be an HTTPS Ethereum JSON-RPC endpoint");
  const executor = normalizedAddress(executorAddress, "EXECUTOR_ADDRESS/BOT_ADDRESS");
  const recipient = normalizedAddress(expectedRecipient, "EXPECTED_PROFIT_RECIPIENT");
  const ownerExpected = expectedOwner
    ? normalizedAddress(expectedOwner, "EXPECTED_OWNER") : null;
  const aavePool = AAVE_V3_ETHEREUM_POOL.toLowerCase();
  const chainId = BigInt(await rpc(rpcUrl, "eth_chainId"));
  if (chainId !== 1n) throw new Error("RPC is not Ethereum mainnet (chainId != 1)");

  const aaveCode = await rpc(rpcUrl, "eth_getCode", [aavePool, "latest"]);
  if (typeof aaveCode !== "string" || aaveCode.length <= 2)
    throw new Error("Official Aave V3 Pool has no code at RPC target");

  const runtimeCode = await rpc(rpcUrl, "eth_getCode", [executor, "latest"]);
  if (typeof runtimeCode !== "string" || !/^0x(?:[\da-fA-F]{2})+$/.test(runtimeCode))
    throw new Error("Executor is not a deployed contract (no on-chain bytecode)");

  const call = (to, data) => rpc(rpcUrl, "eth_call", [{ to, data }, "latest"]);
  const [contractPool, contractOwner, contractRecipient, paused, autoPayout, premiumBps] =
    await Promise.all([
      call(executor, SELECTORS.pool),
      call(executor, SELECTORS.owner),
      call(executor, SELECTORS.recipient),
      call(executor, SELECTORS.paused),
      call(executor, SELECTORS.autoPayout),
      call(aavePool, SELECTORS.premium)
    ]);

  const actualPool = decodeAddress(contractPool, "pool()");
  const actualOwner = decodeAddress(contractOwner, "owner()");
  const actualRecipient = decodeAddress(contractRecipient, "profitRecipient()");
  const isPaused = decodeUInt(paused, "paused()") === 1n;
  const payoutEnabled = decodeUInt(autoPayout, "autoPayout()") === 1n;
  const premium = decodeUInt(premiumBps, "FLASHLOAN_PREMIUM_TOTAL()");
  const runtimeCodeSha256 = createHash("sha256")
    .update(Buffer.from(runtimeCode.slice(2), "hex")).digest("hex");

  const reasons = [];
  if (actualPool !== aavePool) reasons.push("executor.pool() is not the canonical Aave Ethereum V3 Pool");
  if (actualRecipient !== recipient) reasons.push("executor.profitRecipient() differs from expected receiving wallet");
  if (!ownerExpected) reasons.push("EXPECTED_OWNER not supplied; signer ownership is unverified");
  else if (actualOwner !== ownerExpected) reasons.push("executor.owner() does not match expected signer");
  if (!isPaused) reasons.push("executor is not paused; owner controls require review");
  if (premium > 1000n) reasons.push("Aave premium unexpectedly exceeds 10%, verify RPC/contract");

  const pinned = typeof expectedRuntimeCodeSha256 === "string"
    && /^[0-9a-fA-F]{64}$/.test(expectedRuntimeCodeSha256);
  if (!pinned) reasons.push("compiled, independently reviewed runtime bytecode SHA256 was not supplied");
  else if (runtimeCodeSha256 !== expectedRuntimeCodeSha256.toLowerCase())
    reasons.push("deployed executor bytecode does not match expected reviewed build");

  return {
    chainId: Number(chainId),
    officialAavePool: aavePool,
    aavePremiumBps: premium.toString(),
    executor,
    contractOwner: actualOwner,
    configuredRecipient: recipient,
    onchainProfitRecipient: actualRecipient,
    paused: isPaused,
    autoPayoutEnabled: payoutEnabled,
    runtimeCodeSha256,
    wiringVerified: reasons.length === 0,
    // Even a fully verified read-only check does NOT authorize real spending.
    transactionSubmissionEnabled: false,
    reasons
  };
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  try {
    const report = await inspectAaveWiring({
      rpcUrl: process.env.HTTPS_URL,
      executorAddress: process.env.EXECUTOR_ADDRESS || process.env.BOT_ADDRESS,
      expectedRecipient: process.env.EXPECTED_PROFIT_RECIPIENT || process.env.PROFIT_RECIPIENT_ADDRESS,
      expectedOwner: process.env.EXPECTED_OWNER,
      expectedRuntimeCodeSha256: process.env.EXPECTED_RUNTIME_CODE_SHA256
    });
    console.log(JSON.stringify(report, null, 2));
    if (!report.wiringVerified) process.exitCode = 2;
  } catch (error) {
    console.error("FAIL-CLOSED Aave wiring check:", error.message);
    process.exitCode = 2;
  }
}
