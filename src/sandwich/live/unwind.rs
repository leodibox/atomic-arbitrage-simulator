use super::config::SandwichConfig;
use super::execution_helpers::{build_execute_calldata, erc20_balance, sign_execute};
use super::nonce::NonceManager;
use super::relay::{new_uuid, BundleRequest, RelayClient};
use super::universe::Universe;
use super::worker::receipt_success;
use alloy::eips::BlockNumberOrTag;
use alloy::network::Ethereum;
use alloy::primitives::{keccak256, Address, B256, U256};
use alloy::providers::Provider;
use alloy::signers::local::PrivateKeySigner;
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct Position {
    pub token: Address,
    pub pair: Address,
    pub opened_block: u64,
    pub token0_is_main: bool,
    pub amount: U256,
}

pub type Positions = Arc<tokio::sync::Mutex<Vec<Position>>>;

/// An unwind that was submitted but not yet proven by a receipt.
struct Inflight {
    position: Position,
    hash: B256,
    target: u64,
    tries: u32,
}

/// Identity that survives other positions from the same block.
fn key(p: &Position) -> (Address, Address, u64) {
    (p.pair, p.token, p.opened_block)
}

pub fn spawn<P: Provider<Ethereum> + Clone + 'static>(
    provider: P,
    cfg: Arc<SandwichConfig>,
    universe: Arc<Universe>,
    positions: Positions,
    nonces: Arc<NonceManager>,
    relays: Arc<RelayClient>,
    signer: PrivateKeySigner,
) {
    tokio::spawn(async move {
        let mut last = 0u64;
        let mut inflight: Vec<Inflight> = Vec::new();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let head = universe.head();
            if head <= last {
                continue;
            }
            last = head; // Submission is not a sale: settle only on a receipt.
            let taken = std::mem::take(&mut inflight);
            let mut keep: Vec<Inflight> = Vec::new();
            let mut retry: Vec<Position> = Vec::new();
            let mut settled: Vec<(Address, Address, u64)> = Vec::new();
            for mut f in taken {
                if head <= f.target {
                    keep.push(f);
                    continue;
                }
                if receipt_success(&provider, f.hash).await {
                    settled.push(key(&f.position));
                } else if f.tries < 3 {
                    f.tries += 1;
                    retry.push(f.position.clone());
                } else {
                    warn!(token = %f.position.token, "unwind abandoned after retries");
                }
            }
            inflight = keep;
            // A position already being unwound must not be selected again.
            let busy: Vec<(Address, Address, u64)> =
                inflight.iter().map(|f| key(&f.position)).collect();

            let mut due: Vec<Position> = {
                let guard = positions.lock().await;
                guard
                    .iter()
                    .filter(|p| head.saturating_sub(p.opened_block) >= cfg.unwind_after_blocks)
                    .cloned()
                    .collect()
            };
            due.extend(retry);
            due.retain(|p| !busy.contains(&key(p)));

            for p in due {
                let held = match erc20_balance(&provider, p.token, cfg.executor).await {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(%e, "unwind balance read failed");
                        continue;
                    }
                };
                let sell = held.min(p.amount);
                if sell.is_zero() {
                    if head.saturating_sub(p.opened_block) > cfg.unwind_after_blocks + 5 {
                        settled.push(key(&p));
                    }
                    continue;
                }
                let cd = build_execute_calldata(
                    head + 1,
                    U256::ZERO,
                    &[(!p.token0_is_main, p.pair, p.token, sell)],
                );
                let reserved = nonces.reserve(1).await;
                let nonce = match reserved.first() {
                    Some(n) => *n,
                    None => continue,
                };
                let base = provider
                    .get_block_by_number(BlockNumberOrTag::Latest)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|b| b.header.base_fee_per_gas)
                    .map(U256::from)
                    .unwrap_or(U256::from(20_000_000_000u64));
                let cap = cfg.max_gas_price * U256::from(1_000_000_000u64);
                // The fee must not climb past the cap; if the base already sits
                // above it, the base is the floor we cannot go under.
                let ceiling = cap.max(base);
                let tip = (base + (ceiling - base).min(base)).min(ceiling);
                let max_fee = (base * U256::from(3u64)).min(ceiling).max(tip);
                match sign_execute(
                    &signer,
                    cfg.executor,
                    cd,
                    cfg.gas_units,
                    max_fee,
                    tip,
                    nonce,
                ) {
                    Ok(raw) => {
                        let hash = keccak256(&raw);
                        let _ = relays
                            .send_bundle(&BundleRequest {
                                txs: vec![raw],
                                block: head + 1,
                                reverting: Vec::new(),
                                replacement_uuid: new_uuid(),
                            })
                            .await;
                        info!(token = %p.token, "unwind submitted");
                        inflight.push(Inflight {
                            position: p,
                            hash,
                            target: head + 1,
                            tries: 0,
                        });
                    }
                    Err(e) => {
                        nonces.release(&reserved).await;
                        warn!(%e, "unwind signing failed");
                    }
                }
                let _ = nonces.reconcile().await;
            }
            if !settled.is_empty() {
                let mut guard = positions.lock().await;
                guard.retain(|p| !settled.contains(&key(p)));
            }
        }
    });
}
