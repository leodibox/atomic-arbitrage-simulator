#!/usr/bin/env node
// Read-only USDC/WETH Uniswap V2 vs Sushi V2 mainnet cost assessment.
// Zero signing, zero deployment, zero eth_sendRawTransaction and zero approvals.
// Discovery/research ONLY: no on-chain executable gas estimate or trade authorization.
import { pathToFileURL } from 'node:url';

export const ADDR = Object.freeze({
  usdc: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
  weth: '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2',
  uniPair: '0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc',
  sushiPair: '0x397FF1542f962076d0BFE58eA045FfA2d347ACa0',
  uniFactory: '0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f',
  sushiFactory: '0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac',
  aave: '0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2',
  ethUsd: '0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419',
  usdcUsd: '0x8fFfFfd4AfB6115b954Bd326cbe7B4BA576818f6'
});
const SIG = Object.freeze({
  getReserves: '0x0902f1ac', token0: '0x0dfe1681', token1: '0xd21220a7',
  factory: '0xc45a0155', getPair: '0xe6a43905', premium: '0x074b2e43',
  latestRoundData: '0xfeaf968c', decimals: '0x313ce567'
});
const ONE_ETH = 10n ** 18n;
const ONE_USDC = 10n ** 6n;
const ceilDiv = (n, d) => (n + d - 1n) / d;
const wordAddress = (v) => '0'.repeat(24) + v.slice(2).toLowerCase();
const word = (v, index = 0) => {
  if (typeof v !== 'string' || !/^0x[0-9a-fA-F]+$/.test(v) || v.length < 2 + 64 * (index + 1)) {
    throw new Error('Invalid ABI response');
  }
  return BigInt('0x' + v.slice(2 + 64 * index, 2 + 64 * (index + 1)));
};
const decodeAddress = (v) => '0x' + word(v).toString(16).padStart(40, '0');
const sameAddress = (a, b) => a.toLowerCase() === b.toLowerCase();
const hexBlock = (n) => '0x' + n.toString(16);

export function quoteV2(amount, reserveIn, reserveOut) {
  if (amount <= 0n || reserveIn <= 0n || reserveOut <= 0n) throw new Error('Empty amount or reserves');
  return amount * 997n * reserveOut / (reserveIn * 1000n + amount * 997n);
}

export function calculateGasInUsdc({ gasUnits, baseFeeWei, priorityFeeWei, ethUsd, usdcUsd, bufferBps = 2000n }) {
  if ([gasUnits, baseFeeWei, ethUsd.answer, usdcUsd.answer].some(x => x <= 0n) || priorityFeeWei < 0n)
    throw new Error('Invalid gas/oracle inputs');
  if (bufferBps < 0n || bufferBps > 10000n) throw new Error('Invalid buffer');
  const gasWei = gasUnits * (baseFeeWei + priorityFeeWei);
  const gasTokenRaw = ceilDiv(gasWei * ethUsd.answer * ONE_USDC * (10n ** BigInt(usdcUsd.decimals)),
    ONE_ETH * (10n ** BigInt(ethUsd.decimals)) * usdcUsd.answer);
  return { gasWei, gasTokenRaw, gasWithBufferRaw: ceilDiv(gasTokenRaw * (10000n + bufferBps), 10000n) };
}

export function evaluateRoute({ first, second, amountRaw, premiumBps, gasRaw }) {
  if (premiumBps < 0n || premiumBps > 1000n || gasRaw <= 0n) throw new Error('Invalid fee input');
  const intermediateWeth = quoteV2(amountRaw, first.usdcReserve, first.wethReserve);
  const outputUsdc = quoteV2(intermediateWeth, second.wethReserve, second.usdcReserve);
  const premiumRaw = ceilDiv(amountRaw * premiumBps, 10000n); // conservative rounding
  const grossRaw = outputUsdc - amountRaw;
  const preGasRaw = grossRaw - premiumRaw;
  const estimatedPostGasRaw = preGasRaw - gasRaw;
  return { amountRaw, outputUsdc, premiumRaw, grossRaw, preGasRaw, estimatedPostGasRaw,
    theoreticalPositive: estimatedPostGasRaw > 0n,
    approvedForExecution: false };
}

export function parseRound(data, timestamp, maxAgeSeconds) {
  const round = word(data, 0);
  let answer = word(data, 1);
  if (answer >= 1n << 255n) answer -= 1n << 256n;
  const updatedAt = word(data, 3);
  const answeredInRound = word(data, 4);
  if (round === 0n || answeredInRound < round || answer <= 0n || updatedAt === 0n ||
      timestamp < updatedAt || timestamp - updatedAt > BigInt(maxAgeSeconds))
    throw new Error('Oracle answer is invalid or stale at sampled block');
  return { answer, updatedAt };
}

export async function readV2Snapshot(rpc, { blockTag: selectedBlock = 'latest' } = {}) {
  if (!/^(latest|safe|finalized|0x[0-9a-fA-F]+)$/.test(selectedBlock))
    throw new Error('Invalid V2_BLOCK_TAG');
  const chain = BigInt(await rpc('eth_chainId', []));
  if (chain !== 1n) throw new Error('Expected Ethereum Mainnet chain ID 1');
  const block = await rpc('eth_getBlockByNumber', [selectedBlock, false]);
  if (!block?.number || !block?.timestamp || !block?.baseFeePerGas)
    throw new Error('Missing EIP-1559 block metadata');
  const blockNumber = BigInt(block.number);
  if (selectedBlock.startsWith('0x') && blockNumber !== BigInt(selectedBlock))
    throw new Error('RPC returned a different block than requested');
  const blockTag = hexBlock(blockNumber);
  const blockTime = BigInt(block.timestamp);
  const baseFeeWei = BigInt(block.baseFeePerGas);
  const call = (address, data) => rpc('eth_call', [{ to: address, data }, blockTag]);
  async function readPair(pair, factory) {
    const [code, t0, t1, f, reserve, factoryPair] = await Promise.all([
      rpc('eth_getCode', [pair, blockTag]), call(pair, SIG.token0), call(pair, SIG.token1),
      call(pair, SIG.factory), call(pair, SIG.getReserves),
      call(factory, SIG.getPair + wordAddress(ADDR.usdc) + wordAddress(ADDR.weth))
    ]);
    if (!code || code === '0x') throw new Error('Pair has no code');
    if (!sameAddress(decodeAddress(t0), ADDR.usdc) || !sameAddress(decodeAddress(t1), ADDR.weth) ||
        !sameAddress(decodeAddress(f), factory) || !sameAddress(decodeAddress(factoryPair), pair))
      throw new Error('Pair/factory identity does not match canonical USDC/WETH');
    const usdcReserve = word(reserve, 0);
    const wethReserve = word(reserve, 1);
    if (usdcReserve <= 0n || wethReserve <= 0n) throw new Error('Empty pair liquidity');
    return { pair, usdcReserve, wethReserve };
  }
  async function readFeed(address, maxAgeSeconds) {
    const [roundData, decimalsData] = await Promise.all([
      call(address, SIG.latestRoundData), call(address, SIG.decimals)
    ]);
    const decimals = Number(word(decimalsData));
    if (!Number.isInteger(decimals) || decimals < 1 || decimals > 18)
      throw new Error('Unsupported oracle decimals');
    return { ...parseRound(roundData, blockTime, maxAgeSeconds), decimals };
  }
  const [uni, sushi, premium, ethUsd, usdcUsd] = await Promise.all([
    readPair(ADDR.uniPair, ADDR.uniFactory), readPair(ADDR.sushiPair, ADDR.sushiFactory),
    call(ADDR.aave, SIG.premium), readFeed(ADDR.ethUsd, 7200), readFeed(ADDR.usdcUsd, 172800)
  ]);
  const premiumBps = word(premium);
  if (premiumBps > 1000n) throw new Error('Unreasonably large Aave premium');
  return { blockNumber, blockHash: block.hash, blockTime, baseFeeWei, uni, sushi, ethUsd, usdcUsd, premiumBps };
}

export function analyzeSnapshot(snapshot, { gasUnits = 400000n, priorityFeeWei = 1000000000n,
  bufferBps = 2000n, amountsUsdc = [100n, 1000n, 5000n, 10000n] } = {}) {
  const gas = calculateGasInUsdc({ gasUnits, baseFeeWei: snapshot.baseFeeWei,
    priorityFeeWei, ethUsd: snapshot.ethUsd, usdcUsd: snapshot.usdcUsd, bufferBps });
  const evaluated = [];
  for (const [name, a, b] of [
    ['Uniswap V2 → Sushi V2', snapshot.uni, snapshot.sushi],
    ['Sushi V2 → Uniswap V2', snapshot.sushi, snapshot.uni]
  ]) {
    for (const amount of amountsUsdc) {
      const metrics = evaluateRoute({ first: a, second: b, amountRaw: amount * ONE_USDC,
        premiumBps: snapshot.premiumBps, gasRaw: gas.gasWithBufferRaw });
      evaluated.push({ route: name, ...metrics });
    }
  }
  return { blockNumber: snapshot.blockNumber, premiumBps: snapshot.premiumBps, snapshot, gas,
    assumptions: { gasUnits, priorityFeeWei, bufferBps, source: 'V2 reserves and Chainlink oracle at a single sampled block',
      gasEstimateVerified: false, executableCalldataVerified: false, transactionSubmissionEnabled: false },
    routes: evaluated };
}

function usdc(raw) { return (raw < 0n ? '-' : '') +
  (raw < 0n ? -raw : raw).toString().padStart(7, '0').replace(/(\d{6})$/, '.$1'); }
const plain = obj => JSON.parse(JSON.stringify(obj, (_, v) => typeof v === 'bigint' ? v.toString() : v));

export async function jsonRpc(url, method, params = [], fetchFn = fetch) {
  if (typeof url !== 'string' || !/^https:\/\//i.test(url)) throw new Error('Private HTTPS_URL is required');
  let response;
  try { response = await fetchFn(url, { method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }) }); }
  catch { throw new Error('RPC network failure (credential details hidden)'); }
  if (!response.ok) throw new Error('RPC HTTP request failed: ' + response.status);
  let payload;
  try { payload = await response.json(); } catch { throw new Error('RPC returned malformed JSON'); }
  if (payload?.error || payload?.result == null) throw new Error('RPC method failed: ' + method);
  return payload.result;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  try {
    const url = process.env.HTTPS_URL;
    const snapshot = await readV2Snapshot((method, params) => jsonRpc(url, method, params),
      { blockTag: process.env.V2_BLOCK_TAG || 'latest' });
    const report = analyzeSnapshot(snapshot);
    console.log(JSON.stringify({ ...plain(report),
      presentation: report.routes.map(r => ({ route: r.route, inputUSDC: usdc(r.amountRaw),
        outputUSDC: usdc(r.outputUsdc), preGasUSDC: usdc(r.preGasRaw),
        estimatedPostGasUSDC: usdc(r.estimatedPostGasRaw), approvedForExecution: false }))
    }, null, 2));
  } catch (e) { console.error('Read-only assessment failed closed:', e.message); process.exitCode = 2; }
}
