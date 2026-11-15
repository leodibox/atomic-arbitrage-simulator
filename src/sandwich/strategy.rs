use crate::{
    common::{constants::Env, pools::PoolsMap, streams::Event},
    sandwich::{appetizer::is_appetizing, main_dish::optimize_amount, simulation::*},
};
use alloy::network::{Ethereum, TransactionResponse};
use alloy::primitives::U256;
use alloy::providers::Provider;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, warn};

pub async fn run_sandwich_strategy<P: Provider<Ethereum> + Clone + 'static>(
    env: Arc<Env>,
    provider: P,
    pools: Arc<PoolsMap>,
    mut rx: broadcast::Receiver<Event>,
    head: Arc<Mutex<u64>>,
) -> Result<()> {
    let harness = Harness {
        provider: provider.clone(),
        executor: env.bot_address,
        float: env.weth_float,
    };
    loop {
        let Ok(event) = rx.recv().await else { continue };
        let Event::PendingTx(pending) = event else {
            if let Event::Block(b) = event { *head.lock().await = b.number; }
            continue;
        };

        let hash = pending.tx.tx_hash();
        let Some(info) = get_pending_tx_info(&provider, hash, &pools).await? else { continue };
        if info.swaps.is_empty() { continue; }

        for swap in info.swaps {
            let block = *head.lock().await;
            let max_gas = info.gas_price * U256::from(300_000u64);

            let mut probe = |amount: U256| -> Option<U256> {
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(
                        harness.triangle(block, U256::ZERO, &swap, &pending.tx, amount)
                    )
                })
            };

            if !is_appetizing(&mut probe, max_gas) { continue; }

            let cap = env.weth_float.min(U256::from(u128::MAX));
            let Some((amount, profit)) = optimize_amount(probe, U256::ZERO, cap) else { continue };
            let revenue = profit.saturating_sub(max_gas);
            if revenue < env.min_revenue { continue; }

            info!(
                pair = %swap.target_pair, victim = %hash, front = %amount,
                profit = %profit, revenue = %revenue, "opportunity"
            );
            if !env.debug { warn!("submit path not wired — DEBUG=true"); }
        }
    }
}
