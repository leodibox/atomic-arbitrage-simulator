use super::config::{Feed, SandwichConfig};
use alloy::consensus::TxEnvelope;
use alloy::network::Ethereum;
use alloy::network::TransactionResponse;
use alloy::providers::Provider;
use alloy::rpc::types::Transaction;
use anyhow::Result;
use futures::StreamExt;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

const SEEN_CAP: usize = 40_000;

struct Seen {
    set: HashSet<[u8; 32]>,
    order: VecDeque<[u8; 32]>,
}

impl Seen {
    fn new() -> Self {
        Self {
            set: HashSet::new(),
            order: VecDeque::new(),
        }
    }

    fn insert(&mut self, hash: [u8; 32]) -> bool {
        if !self.set.insert(hash) {
            return false;
        }
        self.order.push_back(hash);
        if self.order.len() > SEEN_CAP {
            if let Some(old) = self.order.pop_front() {
                self.set.remove(&old);
            }
        }
        true
    }
}

use alloy::consensus::Transaction as ConsensusTx;

pub async fn run<P>(
    provider: P,
    cfg: Arc<SandwichConfig>,
    out: mpsc::Sender<Transaction<TxEnvelope>>,
) -> Result<()>
where
    P: Provider<Ethereum> + Clone + 'static,
{
    if cfg.feed == Feed::Alchemy {
        let params = (
            "alchemy_pendingTransactions",
            serde_json::json!({
                "hashesOnly": false,
            }),
        );
        match provider
            .subscribe::<_, Transaction<TxEnvelope>>(params)
            .await
        {
            Ok(sub) => {
                info!(
                    routers = cfg.routers.len(),
                    "pending feed: alchemy subscription, filtering locally"
                );
                let mut stream = sub.into_stream();
                let mut arrived: u64 = 0;
                let mut routed: u64 = 0;
                while let Some(tx) = stream.next().await {
                    arrived += 1;
                    if arrived == 1 || arrived % 200 == 0 {
                        info!(arrived, routed, "pending feed: transactions arrived");
                    }
                    let hit = tx
                        .inner
                        .to()
                        .map(|to| cfg.routers.iter().any(|r| *r == to))
                        .unwrap_or(false);
                    if !hit {
                        continue;
                    }
                    routed += 1;
                    if routed == 1 || routed % 10 == 0 {
                        info!(routed, "pending feed: router match");
                    }
                    if out.send(tx).await.is_err() {
                        return Ok(());
                    }
                }
                warn!("alchemy pending stream ended");
            }
            Err(e) => warn!(%e, "alchemy subscription refused, falling back to txpool"),
        }
    }

    if cfg.feed == Feed::FullPending {
        match provider
            .subscribe::<_, Transaction<TxEnvelope>>(("newPendingTransactions", (false,)))
            .await
        {
            Ok(sub) => {
                info!("pending feed: full-body subscription");
                let mut stream = sub.into_stream();
                while let Some(tx) = stream.next().await {
                    if out.send(tx).await.is_err() {
                        return Ok(());
                    }
                }
                warn!("full pending stream ended");
            }
            Err(e) => warn!(%e, "full-body subscription refused, using txpool polling"),
        }
    }

    info!(
        poll_ms = cfg.poll_ms,
        "pending feed: txpool_content polling"
    );
    let mut seen = Seen::new();
    loop {
        let raw: serde_json::Value = match provider.raw_request("txpool_content".into(), ()).await {
            Ok(v) => v,
            Err(e) => {
                warn!(%e, "txpool_content failed");
                tokio::time::sleep(std::time::Duration::from_millis(cfg.poll_ms * 2)).await;
                continue;
            }
        };

        for bucket in ["pending", "queued"] {
            let Some(map) = raw.get(bucket).and_then(|v| v.as_object()) else {
                continue;
            };
            for by_nonce in map.values() {
                let Some(by_nonce) = by_nonce.as_object() else {
                    continue;
                };
                for value in by_nonce.values() {
                    let Ok(tx) = serde_json::from_value::<Transaction<TxEnvelope>>(value.clone())
                    else {
                        continue;
                    };
                    if !seen.insert(tx.tx_hash().0) {
                        continue;
                    }
                    if out.send(tx).await.is_err() {
                        return Ok(());
                    }
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(cfg.poll_ms)).await;
    }
}
