#!/usr/bin/env node
// Destructive integration test ONLY for a dedicated disposable PostgreSQL DB.
// NEVER run against Nimbus, Trading Lab, or the live Sandwich Bot ledger.
// Requires psql CLI and private PGHOST/PGPORT/PGUSER/PGPASSWORD env vars.
// No RPC calls, contract deployment or transactions are made.
import { execFile, execFileSync } from "node:child_process";
import { promisify } from "node:util";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import assert from "node:assert/strict";

const execFileAsync = promisify(execFile);
const testDb = process.env.SANDWICH_TEST_DB_NAME ?? "";
if (!/^sandwich_bot_test_[a-z0-9_]+$/.test(testDb)) {
  console.error("Refusing: SANDWICH_TEST_DB_NAME must start sandwich_bot_test_ and be disposable");
  process.exit(2);
}
const psql = "psql";
const args = ["-X", "-v", "ON_ERROR_STOP=1", "-q", "-A", "-t", "-d", testDb];
function sqlNow(secondsOffset = 0) {
  return `CURRENT_TIMESTAMP + INTERVAL '${secondsOffset} seconds'`;
}
function values(overrides = {}) {
  const base = {
    chain: 1, signer: "'0x1111111111111111111111111111111111111111'",
    executor: "'0x2222222222222222222222222222222222222222'",
    nonce: 0, hash: `'${"a".repeat(64)}'`, gas: 100000,
    fee: 10000000000, fx: 300000000000, quote: sqlNow(), usd: 330000000
  };
  return { ...base, ...overrides };
}
function reservation(o = {}) {
  const v = values(o);
  return `INSERT INTO first_live_trade_attempt
    (trial_id, chain_id, signer_address, executor_address, tx_nonce, calldata_sha256,
      gas_limit, max_fee_per_gas_wei, eth_usd_e8, quote_timestamp, maximum_gas_usd_e8)
    VALUES ('first_live_trial', ${v.chain}, ${v.signer}, ${v.executor}, ${v.nonce},
      ${v.hash}, ${v.gas}, ${v.fee}, ${v.fx}, ${v.quote}, ${v.usd})
    ON CONFLICT (trial_id) DO NOTHING
    RETURNING attempt_id;`;
}
function run(sql) {
  return execFileSync(psql, [...args, "-c", sql], {encoding:"utf8", stdio:["ignore","pipe","pipe"]}).trim();
}
async function parallel(sql) {
  const calls = [0,1].map(() => execFileAsync(psql, [...args, "-c", sql], {encoding:"utf8"}));
  return Promise.all(calls);
}
function reset() { run("TRUNCATE TABLE first_live_trade_receipt, first_live_trade_attempt;"); }
function expectsFailure(sql) {
  assert.throws(() => run(sql), /./);
}
async function main() {
  const here = dirname(fileURLToPath(import.meta.url));
  const migration = readFileSync(resolve(here, "../sql/first_live_trade_attempt.sql"), "utf8");
  // Dedicated test database only; DDL and TRUNCATE require test admin user.
  execFileSync(psql, [...args, "-f", "/dev/stdin"], {input:migration, encoding:"utf8"});
  reset();
  try {
    // Two concurrent starts can never claim distinct attempts.
    const both = await parallel(reservation());
    const successful = both.filter(({stdout}) => /^[0-9a-f-]{36}$/im.test(stdout.trim()));
    assert.equal(successful.length, 1, "exactly one reservation must succeed");
    assert.equal(run("SELECT count(*) FROM first_live_trade_attempt;"), "1");

    // Restart/second attempt is denied permanently.
    assert.equal(run(reservation()), "");
    assert.equal(run("SELECT count(*) FROM first_live_trade_attempt;"), "1");
    reset();
    // DB recalculates the fee, rejects fake costs.
    expectsFailure(reservation({fee:20000000000, usd:330000000}));
    expectsFailure(reservation({fee:20000000000, usd:660000000}));
    // Quotes older than 30 sec or from an implausible future are rejected.
    expectsFailure(reservation({quote:sqlNow(-90)}));
    expectsFailure(reservation({quote:sqlNow(60)}));
    // Exact, valid fee is accepted.
    assert.match(run(reservation()), /^[0-9a-f-]{36}$/i);
    console.log("PASS: atomic concurrency, restart/retry denial, DB-side fee and price rejection");
  } finally {
    reset(); // disposable test DB ONLY
  }
}
main().catch(e => {
  console.error("FAIL: isolated one-shot ledger test:", e.message);
  process.exitCode = 1;
});
