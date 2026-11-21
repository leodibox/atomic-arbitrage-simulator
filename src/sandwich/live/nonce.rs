use alloy::network::Ethereum;
use alloy::primitives::Address;
use alloy::providers::Provider;
use anyhow::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct NonceManager {
    provider: Arc<dyn Provider<Ethereum> + Send + Sync>,
    signer: Address,
    next: AtomicU64,
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
            gate: Mutex::new(()),
        })
    }

    pub async fn reserve(&self, count: u64) -> Vec<u64> {
        let _g = self.gate.lock().await;
        let start = self.next.fetch_add(count, Ordering::SeqCst);
        (start..start + count).collect()
    }

    pub async fn reconcile(&self) -> Result<()> {
        let _g = self.gate.lock().await;
        let on_chain = self.provider.get_transaction_count(self.signer).await?;
        let local = self.next.load(Ordering::SeqCst);
        self.next.store(local.max(on_chain), Ordering::SeqCst);
        Ok(())
    }
}
