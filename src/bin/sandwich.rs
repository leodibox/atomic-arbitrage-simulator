use alloy::consensus::TxEnvelope;
use alloy::network::Ethereum;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Transaction;
use alloy::signers::local::PrivateKeySigner;
use sandooo::sandwich::live::{
    config::SandwichConfig, feed, nonce::NonceManager, relay::RelayClient, universe, unwind, worker,
};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cfg = Arc::new(SandwichConfig::from_env()?);
    if cfg.dry_run {
        warn!("DRY RUN: candidates are logged, nothing is signed");
    }

    let provider = ProviderBuilder::new()
        .connect_ws(WsConnect::new(cfg.wss_url.clone()))
        .await?;
    let chain = provider.get_chain_id().await?;
    anyhow::ensure!(chain == 1, "mainnet only, got {chain}");

    let signer = PrivateKeySigner::from_str(&cfg.owner_key)?;
    let identity = PrivateKeySigner::from_str(&cfg.identity_key)?;

    let universe = universe::build(&provider, cfg.clone()).await?;
    universe::spawn_refresh(provider.clone(), cfg.clone(), universe.clone());
    universe.subscribe_sync(provider.clone()).await;

    let relays = Arc::new(RelayClient::new(Arc::new(cfg.relays.clone()), identity));
    let provider_dyn: Arc<dyn Provider<Ethereum> + Send + Sync> = Arc::new(provider.clone());
    let nonces = Arc::new(NonceManager::new(provider_dyn, signer.address()).await?);
    let positions = Arc::new(tokio::sync::Mutex::new(Vec::new()));

    unwind::spawn(
        provider.clone(),
        cfg.clone(),
        universe.clone(),
        positions.clone(),
        nonces.clone(),
        relays.clone(),
        signer.clone(),
    );

    let (tx_out, rx_out) = mpsc::channel::<Transaction<TxEnvelope>>(2048);
    let feed_provider = provider.clone();
    let feed_cfg = cfg.clone();
    tokio::spawn(async move {
        if let Err(e) = feed::run(feed_provider, feed_cfg, tx_out).await {
            warn!(%e, "pending feed stopped");
        }
    });

    info!(routers = cfg.routers.len(), relays = cfg.relays.len(), "sandwich worker up");
    worker::run(cfg, provider, universe, relays, nonces, positions, signer, rx_out).await
}
