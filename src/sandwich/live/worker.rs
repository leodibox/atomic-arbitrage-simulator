use super::config::SandwichConfig;
use super::execution_helpers::{build_execute_calldata, cycle_open, sign_execute};
use super::nonce::NonceManager;
use super::relay::{new_uuid, BundleRequest, RelayClient};
use super::router::decode;
use super::sim::{confirm, Confirm};
use super::sizing::{plan, slippage_bps};
use super::universe::Universe;
use super::unwind::Position;
use super::unwind::Positions;
use crate::arbitrage::state::PoolState;
use crate::common::constants::WETH;
use alloy::eips::{BlockNumberOrTag, Encodable2718};
use alloy::consensus::Transaction as ConsensusTx;
use alloy::network::{Ethereum, TransactionResponse};
use alloy::primitives::{keccak256, Address, B256, Bytes, U256};
use hex;
use alloy::providers::Provider;
use alloy::consensus::TxEnvelope;
use alloy::rpc::types::Transaction;
use alloy::signers::local::PrivateKeySigner;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tracing::{info, warn};

const MAX_SHARE_BPS: u64 = 2_500;

struct Ctx<P: Provider<Ethereum> + Clone + 'static> {
    cfg: Arc<SandwichConfig>,
    universe: Arc<Universe>,
    relays: Arc<RelayClient>,
    nonces: Arc<NonceManager>,
    positions: Positions,
    signer: PrivateKeySigner,
    provider: P,
}

fn expected_out(pool: &PoolState, buys: bool, amount: U256) -> Option<U256> {
    let (rin, rout) = if buys {
        if pool.token0 == WETH { (pool.reserve0, pool.reserve1) } else { (pool.reserve1, pool.reserve0) }
    } else if pool.token0 == WETH {
        (pool.reserve1, pool.reserve0)
    } else {
        (pool.reserve0, pool.reserve1)
    };
    if rin.is_zero() || rout.is_zero() || amount.is_zero() {
        return None;
    }
    let fee_in = amount.checked_mul(U256::from(997u64))?;
    let num = fee_in.checked_mul(rout)?;
    let den = rin.checked_mul(U256::from(1000u64))?.checked_add(fee_in)?;
    Some(num / den)
}

async fn handle<P: Provider<Ethereum> + Clone + 'static>(ctx: Arc<Ctx<P>>, tx: Transaction<TxEnvelope>) {
    let cfg = &ctx.cfg;
    let Some(to) = tx.to() else { return };
    if !cfg.routers.contains(&to) || tx.input().len() < 4 {
        return;
    }

    let uni = ctx.universe.clone();
    let Some(victim) = decode(tx.input(), tx.value(), |a, b| uni.pair_for(a, b), WETH) else {
        return;
    };

    let Some(other) = other_side(&ctx, &victim).await else { return };
    let (token_in, token_out) = if victim.victim_buys {
        (WETH, other)
    } else {
        (other, WETH)
    };

    if !victim.victim_buys {
        return;
    }
    let victim_size = if victim.exact_out { victim.limit } else { victim.amount_in };
    if victim_size < cfg.min_victim_weth {
        return;
    }

    let base_fee = ctx
        .provider
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await
        .ok()
        .flatten()
        .and_then(|b| b.header.base_fee_per_gas)
        .map(U256::from)
        .unwrap_or_default();    let head = ctx.universe.head();
    let Some(pool) = ctx.universe.fresh_pool(&victim.pair, cfg.max_reserve_age).await else {
        return;
    };

    if !victim.exact_out {
        let Some(out) = expected_out(&pool, victim.victim_buys, victim.amount_in) else { return };
        if slippage_bps(out, victim.limit) < cfg.min_slippage_bps {
            return;
        }
    } else if victim.limit.is_zero() {
        return;
    }

    if cycle_open(&ctx.provider, cfg.executor).await != Some(false) {
        return;
    }

    let float = cfg.max_float_weth;
    let Some(p) = plan(&pool, &victim, float, MAX_SHARE_BPS) else { return };

    let front_dir = victim.token0_is_main;
    let conf = Confirm { executor: cfg.executor, owner: ctx.signer.address(), weth_float: float, token_float: U256::ZERO };
    let Some(sim_profit) = confirm(
        ctx.provider.clone(),
        head,
        base_fee,
        p.profit_weth * U256::from(95u64) / U256::from(100u64),
        &tx,
        victim.pair,
        front_dir,
        p.front_amount,
        p.back_amount,
        token_in,
        token_out,
        &conf,
    )
    .await
    else {
        info!(pair = %victim.pair, "preflight rejected");
        return;
    };
    if sim_profit.is_zero() {
        return;
    }

    let victim_tip = U256::from(tx.max_priority_fee_per_gas().unwrap_or_default());
    let base_fee = ctx
        .provider
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await
        .ok()
        .flatten()
        .and_then(|b| b.header.base_fee_per_gas)
        .map(U256::from)
        .unwrap_or_default();
    let share = sim_profit * U256::from(cfg.tip_bps) / U256::from(10_000u64);
    let cap = sim_profit * U256::from(cfg.max_tip_bps) / U256::from(10_000u64);
    let gas = U256::from(cfg.gas_units);
    let tip = (share / gas)
        .max(victim_tip + victim_tip / U256::from(10u64))
        .min(cap / gas);
    let gas_cost = gas * (base_fee + tip);
    let revenue = sim_profit.saturating_sub(gas_cost);
    if revenue < cfg.min_revenue_weth {
        return;
    }
    let fee_cap = cfg.max_gas_price * U256::from(1_000_000_000u64);
    let max_fee = (base_fee * U256::from(2u64) + tip).min(fee_cap);

    info!(
        pair = %victim.pair, victim = %tx.tx_hash(), front = %p.front_amount,
        profit = %sim_profit, revenue = %revenue, tip = %tip, "candidate"
    );
    if cfg.dry_run {
        return;
    }

    let n = ctx.nonces.reserve(2).await;
    let (n0, n1) = (n[0], n[1]);
    let min_profit = p.profit_weth * U256::from(95u64) / U256::from(100u64);

    let victim_raw = tx.inner.encoded_2718();
    let uuid = new_uuid();
    let _victim_hash = format!("0x{}", hex::encode(tx.tx_hash().as_slice()));

    let mut front = Bytes::new();
    let mut back = Bytes::new();
    let mut target = head + 1;
    let mut landed = false;
    let mut unknown = false;
    for _ in 0..cfg.max_blocks {
        // The contract pins block.number, so every retarget needs fresh bytes.
        let front_cd = build_execute_calldata(
            target,
            U256::ZERO,
            &[(front_dir, victim.pair, token_in, p.front_amount)],
        );
        let back_cd = build_execute_calldata(
            target,
            min_profit,
            &[(!front_dir, victim.pair, token_out, p.back_amount)],
        );
        let Ok(front_raw) = sign_execute(&ctx.signer, cfg.executor, front_cd, cfg.gas_units / 2, max_fee, tip, n0) else {
            warn!("front signing failed");
            break;
        };
        let Ok(back_raw) = sign_execute(&ctx.signer, cfg.executor, back_cd, cfg.gas_units / 2, max_fee, tip, n1) else {
            warn!("back signing failed");
            break;
        };
        front = front_raw;
        back = back_raw;
        let outcomes = ctx
            .relays
            .send_bundle(&BundleRequest {
                txs: vec![front.clone(), victim_raw.clone().into(), back.clone()],
                block: target,
                reverting: vec![],
                replacement_uuid: uuid.clone(),
            })
            .await;
        let accepted = outcomes.iter().filter(|o| o.ok).count();
        if accepted == 0 {
            warn!(block = target, ?outcomes, "no relay accepted");
        } else {
            info!(block = target, accepted, "bundle sent");
        }

        // Do not re-sign over the hashes we still have to verify: wait the
        // target block out, then judge both legs by receipt.
        let mut waited = 0u32;
        while ctx.universe.head() <= target && waited < 6 {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            waited += 1;
        }
        if ctx.universe.head() <= target {
            warn!(block = target, "block never advanced, outcome unknown");
            unknown = true;
            break;
        }
        let front_state = receipt_state(&ctx.provider, keccak256(&front)).await;
        let back_state = receipt_state(&ctx.provider, keccak256(&back)).await;
        if front_state == Some(true) && back_state == Some(true) {
            landed = true;
            break;
        }
        warn!(?front_state, ?back_state, "target block passed without both legs landing");
        if front_state.is_none() || back_state.is_none() {
            unknown = true;
        }
        break;
        target = ctx.universe.head() + 1;
    }

    if !landed {
        ctx.relays.cancel(&uuid, target).await;
        if !unknown {
            ctx.nonces.release(&n).await;
        }
        ctx.nonces.release(&[n0, n1]).await;
        ctx.positions.lock().await.push(Position {
            token: token_out,
            pair: victim.pair,
            opened_block: head,
            token0_is_main: victim.token0_is_main,
            amount: p.back_amount,
        });
        info!(pair = %victim.pair, "window expired, unwind armed");
    }
    let _ = ctx.nonces.reconcile().await;
}

/// Some(true) mined and succeeded, Some(false) mined and reverted or never mined, None when the RPC could not answer.
pub async fn receipt_state<P: Provider<Ethereum>>(provider: &P, hash: B256) -> Option<bool> {
    match provider.get_transaction_receipt(hash).await {
        Ok(Some(receipt)) => Some(receipt.status()),
        Ok(None) => Some(false),
        Err(_) => None,
    }
}

pub async fn receipt_success<P: Provider<Ethereum>>(provider: &P, hash: B256) -> bool {
    match provider.get_transaction_receipt(hash).await {
        Ok(Some(receipt)) => receipt.status(),
        _ => false,
    }
}

async fn other_side<P: Provider<Ethereum> + Clone + 'static>(ctx: &Arc<Ctx<P>>, v: &super::router::Victim) -> Option<Address> {
    let pool = ctx.universe.fresh_pool(&v.pair, u64::MAX).await?;
    let other = if pool.token0 == WETH { pool.token1 } else { pool.token0 };
    if other == WETH { None } else { Some(other) }
}

pub async fn run<P: Provider<Ethereum> + Clone + 'static>(
    cfg: Arc<SandwichConfig>,
    provider: P,
    universe: Arc<Universe>,
    relays: Arc<RelayClient>,
    nonces: Arc<NonceManager>,
    positions: Positions,
    signer: PrivateKeySigner,
    mut rx: mpsc::Receiver<Transaction<TxEnvelope>>,
) -> Result<()> {
    let ctx = Arc::new(Ctx { cfg: cfg.clone(), universe, relays, nonces, positions, signer, provider });
    let gate = Arc::new(Semaphore::new(cfg.max_inflight));
    while let Some(tx) = rx.recv().await {
        let Ok(permit) = gate.clone().acquire_owned().await else { return Ok(()) };
        let ctx = ctx.clone();
        tokio::spawn(async move {
            let _p = permit;
            handle(ctx, tx).await;
        });
    }
    Ok(())
}
