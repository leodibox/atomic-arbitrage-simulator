# Issue #15: bounded read-only discovery probe

This is research, never a production scanner/executor or a trading approval. No signing, bundles, transaction RPC methods, live approvals or deployment scripts exist here. Arbitrary token calls are passive and capped; the optional Foundry settlement probe runs only in an isolated fork and **always rolls back**.

## Reproduction

Use Node 24 (Node's proxy support is needed in this cloud runtime):

```sh
npm ci --prefix research/long-tail --cache /workspace/.cache/npm
npm --prefix research/long-tail test
# Inject HTTPS_URL securely. Never paste its secret value into this command or a report.
NODE_USE_ENV_PROXY=1 node research/long-tail/src/probe.mjs
```

Defaults: finalized Mainnet; last seven days located by timestamp/header binary search; 2,000-block event windows; Uniswap V2/Sushi V2/Uniswap V3 only; canonical WETH anchor; at most 80 stratified markets plus USDC control; at most 20 sized routes; 6,000 read-RPC calls per invocation, 25-second request timeout, 500,000 gas per eth_call. Two workers per market plus two subworkers produce up to four concurrent pool reads. Missing/denied windows and metadata failures remain in evidence; they are never interpreted as zero pools.

For a retained discovery checkpoint, inject `DISCOVERY_CHECKPOINT` with its file path and `RUN_DIR` with a new evidence directory. The program re-reads chain ID and the saved end block header and rejects a different hash. This avoids repeating discovery or silently changing its window. Every pool state/quote is explicit-block. It supports reproducibility only if the provider admits that historical state. A checkpoint is local untrusted research data, not an alternate source of verified liquidity.

`MAX_PAIRS` (2–200), `MAX_SIZED_ROUTES` (1–100) and `MAX_RPC_CALLS` set explicit bounds. Do not adjust them to manufacture profitability. This run expanded the 60-market pilot to all 74 accessible WETH markets using the **same** discovery/end-block checkpoint, because the remaining RPC budget was sufficient. The incomplete seven-day coverage was not hidden.

## Economic model

- Two **distinct** pools; all four canonical Uniswap V3 fee tiers checked for existing counterparts regardless of creation date.
- V2 static constant-product formula with real 30-bp fees and integer rounding. Negative marginal static V2 models are discarded. Continuous stationary point and quadratic break-even bounds are reported only where a positive modeled maximum exists; integer neighbor refinement is not claimed as a proof of the global discrete optimum.
- V3 quotes call canonical QuoterV2 and retain output, final sqrt price, initialized ticks crossed and quote gas. Sizing is a bounded grid; no global optimum or continuous break-even across ticks is fabricated.
- Funding is canonical WETH, so native ETH gas and input share the same 18-decimal unit. No USD peg or ETH-to-token oracle conversion is assumed. Borrowability is only observed cash/flags until specifically proven; Morpho premium assumption is canonical documented behavior, not an invented getter.
- 500,000-gas **assumption**, total gas prices 10/30/60 gwei; not additive priority fees. Funding premium and all gas scenarios stored per size and block. Revert EV uses hypothetical success probabilities 0/25/50/75/100%, never a measured inclusion rate. Unattainable probability thresholds are null, not probabilities above 100%.
- Output flags distinguish identity, reference quote, token settlement and gas. Every route has `executable=false`. UNKNOWN token transfer/owner/proxy risks prevent safety claims. A positive gross quote is not GO-RESEARCH.

## Fork probe

Install verified Foundry/Solc 0.8.24, and forge-std at `contracts/lib/forge-std` (tested commit `15b2df92a05d4d2f76a06653872fefb287e0e01c`).

```sh
cd research/long-tail/fork
# HTTPS_URL is private; set LONGTAIL_CASES_JSON to the absolute retained fork-candidates.json path.
forge test --threads 1 -vvv
```

The fixture checks two actual swap settlements against reference outputs, token transfer balance deltas and canonical factories. It obtains WETH through a real on-fork Morpho loan then intentionally reverts the callback, preserving all original balances even for a positive gross margin. It does **not** prove successful repayment or a sendable transaction. No balances/reserves are fabricated, no local deployment script or signer is used. A missing third candidate is SKIPPED, not passed. Root `contracts/` and Rust are untouched.

See `docs/ISSUE_15_LONG_TAIL_DISCOVERY.md` for this run's NO-GO result, coverage denials and measurements. This is not a claim that all Mainnet markets lack arbitrage.
