import test from 'node:test';
import assert from 'node:assert/strict';
import { quoteV2, calculateGasInUsdc, evaluateRoute, parseRound, analyzeSnapshot, readV2Snapshot, ADDR } from './assess_v2_costs.mjs';

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


function mockMainnetRpc({ mismatch = false, wrongChain = false } = {}) {
  const n = v => v.toString(16).padStart(64, '0');
  const a = v => n(BigInt(v));
  const args = (...items) => '0x' + items.map(n).join('');
  const timestamp = 1_800_000_000n;
  return async (method, params) => {
    if (method === 'eth_chainId') return wrongChain ? '0xaa36a7' : '0x1';
    if (method === 'eth_getBlockByNumber') return {
      number: '0x7b', timestamp: '0x' + timestamp.toString(16), baseFeePerGas: '0x3b9aca00'
    };
    if (method === 'eth_getCode') return '0x6001';
    if (method !== 'eth_call') throw new Error('unexpected method');
    const to = params[0].to.toLowerCase();
    const sig = params[0].data.slice(0, 10);
    if (to === ADDR.aave.toLowerCase() && sig === '0x074b2e43') return args(5n);
    if ([ADDR.uniPair.toLowerCase(), ADDR.sushiPair.toLowerCase()].includes(to)) {
      if (sig === '0x0dfe1681') return '0x' + a(ADDR.usdc);
      if (sig === '0xd21220a7') return '0x' + a(ADDR.weth);
      if (sig === '0xc45a0155') return '0x' + a(
        to === ADDR.uniPair.toLowerCase() ? ADDR.uniFactory : ADDR.sushiFactory);
      if (sig === '0x0902f1ac') return args(1000000000000n, 500n * 10n ** 18n, 0n);
    }
    if (sig === '0xe6a43905') {
      if (to === ADDR.uniFactory.toLowerCase()) return '0x' + a(mismatch ? ADDR.sushiPair : ADDR.uniPair);
      if (to === ADDR.sushiFactory.toLowerCase()) return '0x' + a(ADDR.sushiPair);
    }
    if ([ADDR.ethUsd.toLowerCase(), ADDR.usdcUsd.toLowerCase()].includes(to)) {
      if (sig === '0x313ce567') return args(8n);
      if (sig === '0xfeaf968c') return args(12n,
        to === ADDR.ethUsd.toLowerCase() ? 170000000000n : 99990000n,
        timestamp - 100n, timestamp - 100n, 12n);
    }
    throw new Error('unexpected contract call');
  };
}

test('read-only RPC snapshot uses one mainnet block, canonical factories and real Aave getter', async () => {
  const snap = await readV2Snapshot(mockMainnetRpc());
  assert.equal(snap.blockNumber, 123n);
  assert.equal(snap.premiumBps, 5n);
  assert.equal(snap.ethUsd.decimals, 8);
  assert.equal(snap.usdcUsd.answer, 99990000n);
  assert.equal(analyzeSnapshot(snap).routes.length, 8);
});

test('read-only RPC verification fails closed for wrong chain or wrong V2 factory', async () => {
  await assert.rejects(() => readV2Snapshot(mockMainnetRpc({ wrongChain: true })), /Mainnet/);
  await assert.rejects(() => readV2Snapshot(mockMainnetRpc({ mismatch: true })), /identity/);
});

test('explicit historical block pins all state reads and retains auditable snapshot', async () => {
  const base = mockMainnetRpc();
  const calls = [];
  const snap = await readV2Snapshot(async (method, params) => {
    calls.push([method, params]);
    return base(method, params);
  }, { blockTag: '0x7b' });
  assert.deepEqual(calls.find(([m]) => m === 'eth_getBlockByNumber')[1], ['0x7b', false]);
  assert.ok(calls.filter(([m]) => ['eth_call', 'eth_getCode'].includes(m)).every(([, p]) => p[1] === '0x7b'));
  const report = analyzeSnapshot(snap);
  assert.equal(report.snapshot.uni.usdcReserve, 1000000000000n);
  assert.equal(report.snapshot.ethUsd.answer, 170000000000n);
  assert.equal(report.snapshot.blockTime, 1800000000n);
});

test('invalid or mismatched historical blocks fail closed', async () => {
  await assert.rejects(() => readV2Snapshot(mockMainnetRpc(), { blockTag: '0x7c' }), /different block/);
  await assert.rejects(() => readV2Snapshot(mockMainnetRpc(), { blockTag: 'private-url' }), /Invalid V2_BLOCK_TAG/);
});
