import test from 'node:test';
import assert from 'node:assert/strict';
import { quoteV2, calculateGasInUsdc, evaluateRoute, parseRound, analyzeSnapshot } from './assess_v2_costs.mjs';

const p = { usdcReserve: 1000000000000n, wethReserve: 500n * 10n ** 18n };
const q = { usdcReserve: 1000000000000n, wethReserve: 250n * 10n ** 18n };
const oracle = { answer: 170000000000n, decimals: 8 };
const stable = { answer: 100000000n, decimals: 8 };

test('V2 fee and slippage decrease roundtrip returns in equal pools', () => {
  const out = quoteV2(100000000n, p.usdcReserve, p.wethReserve);
  const back = quoteV2(out, p.wethReserve, p.usdcReserve);
  assert.ok(back < 100000000n);
});

test('revenue in USDC is never obtained by treating ETH wei as USDC raw units', () => {
  const cost = calculateGasInUsdc({ gasUnits: 400000n, baseFeeWei: 1000000000n,
    priorityFeeWei: 1000000000n, ethUsd: oracle, usdcUsd: stable, bufferBps: 2000n });
  assert.equal(cost.gasWei, 800000000000000n);
  assert.equal(cost.gasTokenRaw, 1360000n); // $1.36
  assert.equal(cost.gasWithBufferRaw, 1632000n); // $1.632 incl 20% buffer
});

test('real-like V2 route loses money before and after gas', () => {
  const x = evaluateRoute({ first:p, second:p, amountRaw:100000000n,
    premiumBps:5n, gasRaw:1632000n });
  assert.ok(x.preGasRaw < 0n);
  assert.ok(x.estimatedPostGasRaw < x.preGasRaw);
  assert.equal(x.approvedForExecution, false);
});

test('synthetically positive route still is never approved for execution', () => {
  const x = evaluateRoute({ first:p, second:q, amountRaw:10000000000n,
    premiumBps:5n, gasRaw:1632000n });
  assert.ok(x.theoreticalPositive);
  assert.equal(x.approvedForExecution, false);
});

test('reject stale Chainlink round and answeredInRound below round', () => {
  const abi = (v) => v.toString(16).padStart(64, '0');
  const data = '0x' + [3n, 170000000000n, 1n, 1000n, 3n].map(abi).join('');
  assert.equal(parseRound(data, 1200n, 300).answer, 170000000000n);
  assert.throws(() => parseRound(data, 2000n, 300), /stale/);
  const oldRound = '0x' + [3n, 170000000000n, 1n, 1000n, 2n].map(abi).join('');
  assert.throws(() => parseRound(oldRound, 1100n, 300), /invalid/);
});

test('report rejects invalid gas inputs and never sends transaction', () => {
  assert.throws(() => calculateGasInUsdc({ gasUnits:0n, baseFeeWei:1n,
    priorityFeeWei:1n, ethUsd:oracle, usdcUsd:stable }), /Invalid/);
  const report = analyzeSnapshot({ blockNumber: 1n, baseFeeWei: 1n,
    premiumBps: 5n, uni:p, sushi:q, ethUsd:oracle, usdcUsd:stable });
  assert.equal(report.routes.length, 8);
  assert.equal(report.assumptions.gasEstimateVerified, false);
  assert.equal(report.assumptions.transactionSubmissionEnabled, false);
  assert.ok(report.routes.every(x => x.approvedForExecution === false));
});
