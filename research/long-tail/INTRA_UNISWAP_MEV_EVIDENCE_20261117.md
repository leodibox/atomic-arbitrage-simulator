# Issue #15 — Evidence review: intra-Uniswap, MEV economics and execution venue (2026-11-17)

**Research-only addendum.** Sources were inspected before authoring this document. This does not replace the [transaction-conditioned preregistration](TRANSACTION_TRIGGERED_EXPERIMENT.md), change its precommitted criteria, or reclassify any result in PR #16. PRs #10–#17 stay DRAFT/unmerged; no new market sample, fork replay, transaction submission, deployment, key handling, or spending occurred.

## Sources examined

1. Bitquery Research, **Crypto Arbitrage in 2026: 139M Trades, 69 Cents Each**, verified through 2026-08-31, https://bitquery.io/investigations/crypto-arbitrage-69-cents
2. 0xapriori, **State of MEV: March 2026** (report dated March 3; builders/relays, long-/short-tail classification), https://github.com/0xapriori/ethereum-reports/blob/main/dexes/state-of-mev-march-2026.md
3. Flashbots, **MEV and the Limits of Scaling**, https://writings.flashbots.net/mev-and-the-limits-of-scaling
4. Ethereum Foundation, **Glamsterdam on Hoodi Postponed** (2026-10-22), https://blog.ethereum.org/2026/10/22/glamsterdam-hoodi-delay
5. Flashbots current bundle endpoint reference, https://docs.flashbots.net/flashbots-auction/advanced/rpc-endpoint and BuilderNet API, https://buildernet.org/docs/api

## What is supported, and what is NOT supported

| Claim | Source observation | Implication / limitation |
| --- | --- | --- |
| **78.0% of Ethereum arbitrage routes are classified “inside Uniswap alone”** | Bitquery, a specific **30-day** sample and its protocol classifier | Prioritize **same-protocol pool pairs** and triangles in research. This is **trade count**, not profit share or proof that all such trades are specifically V3 1bp↔5bp |
| Ethereum arbitrage P&L split: **49.3% builder, 9.4% burned, 41.3% operator** | Bitquery, one 30-day sample | Separately model network gas/basefee, tip/coinbase/builder payment, and operator result. This split is **aggregate descriptive**, not a 40% bidding instruction, and can omit out-of-band payments |
| **99.5%+ bribes on short-tail MEV vs 0–80% on long-tail** | 0xapriori qualitative taxonomy including **arb, sandwich and liquidation** among short-tail | Different definition, timeframe and population from Bitquery. Cannot treat it as a universal arb bribe or conclude “untracked = uncontested”; any untracked route can have sophisticated private competitors |
| **350 unsuccessful attempts per success, 132M gas** | Flashbots analysis of a **specific Base bot**, not all arbitrageurs | Never normalize high paid-on-chain failure frequency as inevitable or profitable for this bot. Differentiate free offchain simulation/unlanded private bundle vs onchain failed transaction costs |
| Ethereum 0.523% 12-month margin on capital; $7.74/trade in a different multi-year comparison | Bitquery reports two different methodologies | Do not equate capital return, owner profit, net EV, or future trade capture. Respect Bitquery's own warning about unrealized apparent profits and incomplete pool coverage |
| **ePBS/Glamsterdam is not activated on mainnet** | EF postponed Hoodi (2026-10-22), without setting a mainnet date | Design a transport interface later with separate adapters; never assume relay endpoints were deprecated |
| Current Flashbots `eth_sendBundle` endpoint documented | Flashbots API docs | Bundle methods exist at publication, but no connectivity, entitlement, inclusion, or current multi-builder reach was tested from this repo |
| Titan/BuilderNet/Quasar shares | 0xapriori **February 2026 builder** estimates; `~3.5%` was **Flashbots relay** share | Builder and relay shares have different denominators; **a 3.5% relay share does not prove Flashbots bundle reach = 3.5% of blocks**. Check contemporary builder endpoint rules and routing, not historical rank alone |

## Research priority shift — on the SAME Ethereum chain

**Priority A: intra-Uniswap V3 fee-tier routes at distinct pools**. Use canonical factory `getPool(tokenA, tokenB, 100/500/3000/10000)` (where each pool exists), including WETH/USDC and other established, verifiable pairs. Probe V3 1bp ↔ V3 5bp, V3 5bp ↔ V3 30bp, V3 ↔ Uniswap V2 30bp, and **both directions**. Never treat a same-pool reverse swap as independent arbitrage. PR #14 already verified the 1bp/5bp USDC/WETH pools functionally but their five sampled roundtrips were negative; this is not a new profit certification.

**Priority B: same-protocol 3-hop triangles** (token A → B → C → A), capped DFS, real factory identity, per-edge fee/ticks, no repeated self-pool unless sequential shared-pool state is simulated correctly. Use authoritative post-transaction venue states and tick-exact quotes, not an arbitrary end-of-block snapshot.

**Priority C: long-tail with actual counterpart liquidity**. Use both `PairCreated`/`PoolCreated` and volatile `Swap` triggers, including pools with lower or higher liquidity in **explicit depth bands**, but only accept reproducible token code, reserves/ticks, risk and funding. A missing second venue is an absence of a 2-pool cycle, **not** an absence of a possible triangle through historical pools. Pair creation alone is not a trade signal; newly created contracts are high-risk and require FOT/honeypot/blacklist checks in an isolated fork before any research candidate is marked `SETTLEMENT_VALIDATED`. No autonomous sniping module is authorized.

**Priority D: cross-venue and cross-chain** only as measured comparisons. The report does not establish that all intra-protocol trades are more profitable than all cross-protocol trades or that Ethereum is always optimal under our infrastructure cost profile; keep alternatives open until statistically comparable samples exist. Uniswap V4 custom hooks are a later, independently audited research scope, not plug-in V3 pool mathematics.

All four priorities are **research strata within the existing preregistered transaction-triggered experiment**, not permission to cherry-pick a winner. Preserve the original A/B sample and stop-rule denominators; report these new route classes separately (and explicitly disclose if pool-route inclusion changes sample composition). Do not alter stop thresholds after seeing results.

## Opportunity-economics fields required for EACH verified candidate

- Transaction-boundary `chainId, blockNumber, blockHash, triggerTxHash, triggerTxIndex, end-of-trigger state proof` and pool addresses/fees
- `notionalWei, grossAfterSwapsWei, flashFeeWei, quoteExactVerified, fundingValidated, tokenRisk, gasUnitsCold` and **historical** `baseFeePerGas`
- `priorityFeeWei, coinbaseTransferWei, builderOrRelayPaymentWei, otherInclusionCostsWei, failedAttemptGasWei` with no double counting: a priority tip is already part of effective gas paid
- `netAfterGasAndPaymentWei` using explicit inclusion assumptions; `operatorRetentionPct` only after actual wallet/strategy reconciliation, not a fixed 41.3% lookup
- `pInclusion` / `pSuccess` **UNKNOWN** until measured on our own opportunity set. For analysis only, show bribe/payments at **0%, 20%, 40%, 60%, 80%, 95%** of positive pre-inclusion surplus, alongside a no-profit case. No fixed “bid 40%”. Simulate probability/sensitivity separately, not as facts.
- Clear states: `DISCOVERED`, `QUOTE_POSITIVE`, `POST_GAS_POSITIVE_SIMULATED`, `SETTLEMENT_VALIDATED`, `INCLUSION_UNVERIFIED`, `OPERATOR_REALIZED_PNL`. **Never** label hypothetical profit `verified positive EV` absent empirically validated inclusion/failure probability.

**Wallet reconciliation** on eventual authorized live trading must track actual pre/post balances across ALL involved tokens, external deposits/withdrawals, transfers between strategy wallets, allowances, gas (burn + tip), builder/relay or validator payments and token valuation at a specified timestamp. A high gross quoted return is not a realized profit. This requirement does not imply permission to conduct live trading.

## Data sourcing, cost, and transport constraints

- PR #17 confirmed the current public RPC denies even **one old block's** log and separate state calls with HTTP 403. Historical `eth_getLogs` coverage and transaction-position state are different capabilities. Do not force 100% backfill or pay for archive/trace access without a small reproducible capability test and explicit cost approval.
- Prefer event-conditioned sampling (largest swaps by size/volatility), with a **transaction boundary after the entire trigger transaction**, not between its internal events; for each candidate require trace/replay or equivalent, which is not supplied by `eth_getLogs` alone.
- Implementing `eth_sendBundle`/Titan/BuilderNet/Quasar endpoints is **out of scope** until a simulated net-positive opportunity is reproduced. If that happens, design sender transport as a pluggable interface with verified vendor-specific methods (not hardwired equal protocol support or reach); ePBS change must be handled at transport/version level.
- Keep scanner Rust, Render, signer, contracts, relays, wallets, funds and onchain transaction submission **untouched**.

## GO/NO-GO remains pre-registered

Follow `TRANSACTION_TRIGGERED_EXPERIMENT.md`: `GO_TO_MORE_RESEARCH` only after **two independently replayed, post-gas-positive transactions** with valid state/settlement and adequate risk evidence. This is **not** an execution authorization and still does **not** establish realized positive EV. `STOP_THIS_STRATUM` only at the existing adequate-replay evidence threshold; insufficient trace access means `INCONCLUSIVE`, not a falsely negative market.
