# Sandwich MEV worker

Separate binary. Nothing in the arbitrage path changes: src/arbitrage/*, src/main.rs and src/sandwich/{strategy,simulation}.rs stay byte-identical.

## What is reused from the scanner

- arbitrage::rpc::discover_weth_v2 -> bounded, factory-verified WETH pair universe
- arbitrage::rpc::load_allowlisted_reserves -> reserve snapshot
- arbitrage::rpc::verify_allowlisted_tokens -> token-order proof before trading
- arbitrage::state::ReserveCache + apply_sync_log -> live reserves from Sync
- arbitrage::state::sync_topic -> subscription filter

## Pipeline

1. pending tx feed (txpool_content poll, or full-body subscription)
2. cheap filters: router allowlist, WETH in path, victim size floor
3. local calldata decode -> pair, direction, amount, victim slippage limit
4. universe lookup + reserve freshness (max_reserve_age_blocks)
5. slippage gate: victim tolerance must exceed MIN_SLIPPAGE_BPS
6. sizing: largest front-run the victim still survives, then profit optimisation
7. REVM preflight of the exact front/victim/back sequence at head state
8. tip = TIP_BPS of profit capped at MAX_TIP_BPS, maxFee = 2*baseFee + tip
9. atomic bundle [front, victimRaw, back] to every relay
10. retarget per head up to MAX_BLOCKS, then cancel via replacementUuid

## Why the victim raw tx is in the bundle

A 2-tx bundle is not a sandwich. Including the victim signed bytes puts
front/victim/back in order in one block. Atomicity is the relay job: the bundle
is submitted with an EMPTY revertingTxHashes, so a reverted victim or a reverted
back-run drops the whole bundle and costs no gas. The contract does not and
cannot enforce cross-transaction atomicity on its own.

## Hard rules in the contract

- block.number must equal the encoded block, so no trading on stale reserves
- the opening leg must pass minProfitWeth = 0 and stores the cycle base
- the closing leg compares against that stored base, so a cycle that ends below
  the base reverts unless minProfitWeth is 0
- minProfitWeth = 0 on the closing leg closes at any result; the unwind relies
  on it, while the normal back-run always passes a real minimum
- onlyOwner plus reentrancy lock
- only the gain above the cycle base is swept; the float is never swept
- resetCycle() clears a dangling cycle but does not sell; sweepAll recovers tokens

## Expected reality

Mainnet V2 sandwiching is contested. With 3 WETH float and a public node expect
a handful of landed bundles per day, not per hour. Real levers: more float, a
low-latency node with txpool, tighter relay coverage.

## Deploy

forge create contracts/src/SandwichExecutor.sol:SandwichExecutor --rpc-url $RPC --private-key $PK

Fund the executor with WETH (the float). SANDWICH_DRY_RUN=true first.
