import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
const sql=readFileSync(new URL("../sql/first_live_trade_attempt.sql",import.meta.url),"utf8");

test("Postgres PL/pgSQL function uses balanced $$ delimiters",()=>{
  const opens=(sql.match(/\bAS \$\$/g)||[]).length;
  const closes=(sql.match(/END \$\$;/g)||[]).length;
  assert.equal(opens,1);
  assert.equal(closes,1);
  assert.doesNotMatch(sql,/\bAS \$\n|END \$;/);
});
test("ledger always remains scoped to isolated sandwich_bot schema",()=>{
  assert.match(sql,/CREATE SCHEMA IF NOT EXISTS sandwich_bot;/);
  assert.match(sql,/CREATE TABLE IF NOT EXISTS sandwich_bot\.first_live_trade_attempt/);
  assert.match(sql,/CREATE TABLE IF NOT EXISTS sandwich_bot\.first_live_trade_receipt/);
  assert.match(sql,/BEFORE INSERT ON sandwich_bot\.first_live_trade_attempt/);
  assert.match(sql,/REFERENCES sandwich_bot\.first_live_trade_attempt/);
  assert.match(sql,/ON CONFLICT \(trial_id\) DO NOTHING/);
});
test("unique one-use trial and worst case USD constraints remain",()=>{
  assert.match(sql,/trial_id TEXT PRIMARY KEY/);
  assert.match(sql,/CHECK \(trial_id = 'first_live_trial'\)/);
  assert.match(sql,/maximum_gas_usd_e8 <= 500000000/);
  assert.match(sql,/quote_timestamp < clock_timestamp\(\) - INTERVAL '30 seconds'/);
});
