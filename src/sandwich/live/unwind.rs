use super::config::SandwichConfig;
use super::execution_helpers::{build_execute_calldata, erc20_balance, sign_execute};
use super::nonce::NonceManager;
use super::relay::{new_uuid, BundleRequest, RelayClient};
use super::universe::Universe;
use alloy::eips::BlockNumberOrTag;
use alloy::network::Ethereum;
use alloy::primitives::{Address, U256};
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
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let head = universe.head();
            if head <= last {
                continue;
            }
            last = head;

            let due: Vec<Position> = {
                let guard = positions.lock().await;
                guard
                    .iter()
                    .filter(|p| head.saturating_sub(p.opened_block) >= cfg.unwind_after_blocks)
                    .cloned()
                    .collect()
            };
            let mut settled_keys: Vec<u64> = Vec::new();

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
                        settled_keys.push(p.opened_block);
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
                let tip = base.min(cap);
                let max_fee = (base * U256::from(3u64)).min(cap).max(base);
                match sign_execute(&signer, cfg.executor, cd, cfg.gas_units, max_fee, tip, nonce) {
                    Ok(raw) => {
                        let _ = relays
                            .send_bundle(&BundleRequest {
                                txs: vec![raw],
                                block: head + 1,
                                reverting: Vec::new(),
                                replacement_uuid: new_uuid(),
                            })
                            .await;
                        info!(token = %p.token, "unwind submitted");
                        settled_keys.push(p.opened_block);
                    }
                    Err(e) => {
                        nonces.release(&reserved).await;
                        warn!(%e, "unwind signing failed");
                    }
                }
                let _ = nonces.reconcile().await;
            }
            if !settled_keys.is_empty() {
                let mut guard = positions.lock().await;
                guard.retain(|p| !settled_keys.contains(&p.opened_block));
            }
        }
    });
}
