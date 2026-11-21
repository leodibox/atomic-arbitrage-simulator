use alloy::{
    network::Ethereum,
    primitives::U256,
    providers::Provider,
    rpc::types::Transaction,
};
use anyhow::Result;
use futures::StreamExt;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub struct NewBlock {
    pub number: u64,
    pub base_fee: U256,
    pub next_base_fee: U256,
}

#[derive(Debug, Clone)]
pub struct NewPendingTx {
    pub tx: Transaction,
}

#[derive(Debug, Clone)]
pub enum Event {
    Block(NewBlock),
    PendingTx(NewPendingTx),
}

pub async fn stream_new_blocks<P: Provider<Ethereum>>(
    provider: P,
    tx: broadcast::Sender<Event>,
) -> Result<()> {
    let mut sub = provider.subscribe_blocks().await?.into_stream();
    while let Some(block) = sub.next().await {
        let base_fee = block.inner.base_fee_per_gas.unwrap_or_default();
        let next_base_fee = crate::common::utils::calculate_next_block_base_fee(
            block.inner.gas_used,
            block.inner.gas_limit,
            base_fee,
        );
        let _ = tx.send(Event::Block(NewBlock {
            number: block.inner.number,
            base_fee: U256::from(base_fee),
            next_base_fee: U256::from(next_base_fee),
        }));
    }
    Ok(())
}

pub async fn stream_pending_transactions<P: Provider<Ethereum>>(
    provider: P,
    tx: broadcast::Sender<Event>,
) -> Result<()> {
    let mut sub = provider.subscribe_pending_transactions().await?.into_stream();
    while let Some(hash) = sub.next().await {
        if let Ok(Some(txn)) = provider.get_transaction_by_hash(hash).await {
            let _ = tx.send(Event::PendingTx(NewPendingTx { tx: txn }));
        }
    }
    Ok(())
}
