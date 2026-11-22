use alloy::network::Ethereum;
use alloy::primitives::Address;
use alloy::providers::Provider;
use anyhow::Result;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct NonceManager {
    provider: Arc<dyn Provider<Ethereum> + Send + Sync>,
    signer: Address,
    next: AtomicU64,
    live: Mutex<BTreeSet<u64>>,
    gate: Mutex<()>,
}

impl NonceManager {
    pub async fn new(
        provider: Arc<dyn Provider<Ethereum> + Send + Sync>,
        signer: Address,
    ) -> Result<Self> {
        let on_chain = provider.get_transaction_count(signer).await?;
        Ok(Self {
            provider,
            signer,
            next: AtomicU64::new(on_chain),
            live: Mutex::new(BTreeSet::new()),
            gate: Mutex::new(()),
        })
    }

    /// Reserve consecutive nonces and keep them outstanding until released or
    /// proven consumed by reconcile.
    pub async fn reserve(&self, count: u64) -> Vec<u64> {
        let _g = self.gate.lock().await;
        let start = self.next.fetch_add(count, Ordering::SeqCst);
        let reserved: Vec<u64> = (start..start + count).collect();
        let mut live = self.live.lock().await;
        for nonce in &reserved {
            live.insert(*nonce);
        }
        reserved
    }

    /// Give back nonces that were never broadcast, so no gap survives.
    pub async fn release(&self, nonces: &[u64]) {
        let mut live = self.live.lock().await;
        for nonce in nonces {
            live.remove(nonce);
        }
    }

    /// Drop resolved nonces and point the cursor at the first non-outstanding
    /// nonce. Taking a maximum against the chain would preserve gaps forever.
    pub async fn reconcile(&self) -> Result<()> {
        let _g = self.gate.lock().await;
        let on_chain = self.provider.get_transaction_count(self.signer).await?;
        let mut live = self.live.lock().await;
        live.retain(|nonce| *nonce >= on_chain);
        let mut candidate = on_chain;
        while live.contains(&candidate) {
            candidate += 1;
        }
        self.next.store(candidate, Ordering::SeqCst);
        Ok(())
    }
}
