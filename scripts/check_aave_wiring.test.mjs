import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { inspectAaveWiring, AAVE_V3_ETHEREUM_POOL } from "./check_aave_wiring.mjs";

const RECIPIENT = "0x49826Aa476d6388fcdccAD7b3661597986a53132";
const OWNER = "0x1111111111111111111111111111111111111111";
const EXECUTOR = "0x2222222222222222222222222222222222222222";
const CONTRACT_CODE = "0x60016000";
const CODE_HASH = createHash("sha256")
  .update(Buffer.from(CONTRACT_CODE.slice(2), "hex")).digest("hex");
const word = address => "0x" + "0".repeat(24) + address.slice(2).toLowerCase();
const int = n => "0x" + BigInt(n).toString(16).padStart(64, "0");

function configuration(overrides = {}) {
  return {
    rpcUrl: "https://placeholder.example",
    executorAddress: EXECUTOR,
    expectedRecipient: RECIPIENT,
    expectedOwner: OWNER,
    expectedRuntimeCodeSha256: CODE_HASH,
    ...overrides
  };
}

function fakeRpc(overrides = {}) {
  return async (_url, method, params = []) => {
    if (method === "eth_chainId") return overrides.chainId ?? "0x1";
    if (method === "eth_getCode") {
      if (params[0].toLowerCase() === EXECUTOR.toLowerCase())
        return overrides.contractCode ?? CONTRACT_CODE;
      if (params[0].toLowerCase() === AAVE_V3_ETHEREUM_POOL.toLowerCase())
        return "0x60026000";
      throw new Error("unexpected contract target");
    }
    if (method === "eth_call") {
      switch (params[0].data) {
        case "0x074b2e43": return int(5);
        case "0x8da5cb5b": return word(overrides.owner ?? OWNER);
        case "0x7e2bad1e": return word(overrides.recipient ?? RECIPIENT);
        case "0x16f0115b": return word(overrides.pool ?? AAVE_V3_ETHEREUM_POOL);
        case "0x5c975abb": return overrides.noPause ? "0x" : int(1);
        case "0x7764db39": return int(overrides.autoPayout ? 1 : 0);
      }
    }
    throw new Error("unexpected JSON-RPC request");
  };
}

test("read-only verifier confirms pinned mainnet wiring but cannot enable trades", async () => {
  const report = await inspectAaveWiring(configuration(), fakeRpc());
  assert.equal(report.wiringVerified, true);
  assert.equal(report.transactionSubmissionEnabled, false);
  assert.equal(report.paused, true);
  assert.equal(report.onchainProfitRecipient, RECIPIENT.toLowerCase());
  assert.equal(report.aavePremiumBps, "5");
});

test("recipient mismatch is a hard verification failure", async () => {
  const report = await inspectAaveWiring(configuration(), fakeRpc({
    recipient: "0x3333333333333333333333333333333333333333"
  }));
  assert.equal(report.wiringVerified, false);
  assert.match(report.reasons.join(" "), /profitRecipient/);
});

test("deployment missing bytecode fails closed", async () => {
  await assert.rejects(
    () => inspectAaveWiring(configuration(), fakeRpc({ contractCode: "0x" })),
    /not a deployed contract/
  );
});

test("wrong chain fails before inspecting contracts", async () => {
  await assert.rejects(
    () => inspectAaveWiring(configuration(), fakeRpc({ chainId: "0xaa36a7" })),
    /not Ethereum mainnet/
  );
});

test("unverified owner or bytecode cannot be treated as connected", async () => {
  const report = await inspectAaveWiring(
    configuration({ expectedOwner: undefined, expectedRuntimeCodeSha256: undefined }),
    fakeRpc()
  );
  assert.equal(report.wiringVerified, false);
  assert.equal(report.reasons.length, 2);
});

test("legacy contract without paused() fails closed", async () => {
  await assert.rejects(
    () => inspectAaveWiring(configuration(), fakeRpc({ noPause: true })),
    /paused\(\) getter/
  );
});
