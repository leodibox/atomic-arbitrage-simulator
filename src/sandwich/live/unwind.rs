use super::config::SandwichConfig;
use super::execution_helpers::{build_execute_calldata, erc20_balance, sign_execute};
use super::nonce::NonceManager;
use super::relay::{new_uuid, BundleRequest, RelayClient};
use super::universe::Universe;
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

            let mut due = Vec::new();
            {
                let mut guard = positions.lock().await;
                let mut keep = Vec::new();
                for p in guard.drain(..) {
                    if head.saturating_sub(p.opened_block) >= cfg.unwind_after_blocks {
                        due.push(p);
                    } else {
                        keep.push(p);
                    }
                }
                *guard = keep;
            }

            for p in due {
                let held = match erc20_balance(&provider, p.token, cfg.executor).await {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(%e, "unwind balance read failed");
                        continue;
                    }
                };
                if held.is_zero() {
                    continue;
                }
                let cd = build_execute_calldata(
                    head + 1,
                    U256::ZERO,
                    &[(!p.token0_is_main, p.pair, p.token, held)],
                );
                let nonce = match nonces.reserve(1).await.first() {
                    Some(n) => *n,
                    None => continue,
                };

                let base = U256::from(20_000_000_000u64);
                match sign_execute(&signer, cfg.executor, cd, cfg.gas_units, base * U256::from(2u64), base, nonce) {
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
                    }
                    Err(e) => warn!(%e, "unwind signing failed"),
                }
                let _ = nonces.reconcile().await;
            }
        }
    });
}
