use alloy::primitives::{address, Address, U256};
use anyhow::{Context, Result};
use std::env;

pub const PROJECT_NAME: &str = "sandooo";
pub const COINBASE: Address = address!("dafea492d9c6733ae3d56b7ed1adb60692a5873e");
pub const WETH: Address = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
pub const FACTORY_V2: Address = address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f");
pub const WETH_DECIMALS: u32 = 18;
pub const WETH_BALANCE_SLOT: u64 = 3;

pub const V2_SWAP_EVENT_ID: [u8; 4] = [0xd7, 0x8a, 0xd9, 0x5f];
pub const PAIR_CREATED_TOPIC: &str =
    "0x0d3648bd0f6ba80134a33ba9275ac585d9d315f0ad8355cddefde31afa28d0e9";
pub const POOL_SCAN_START_BLOCK: u64 = 10_000_000;
pub const POOL_SCAN_BATCH: u64 = 50_000;

#[derive(Debug, Clone)]
pub struct Env {
    pub https_url: String,
    pub wss_url: String,
    pub bot_address: Address,
    /// Optional by design: simulation mode never signs or submits transactions.
    pub private_key: Option<String>,
    pub identity_key: String,
    pub telegram_token: Option<String>,
    pub telegram_chat_id: Option<String>,
    pub use_alert: bool,
    pub debug: bool,
    pub min_revenue: U256,
    pub bribe_bps: u64,
    pub max_gas_price: U256,
    pub weth_float: U256,
    pub profit_recipient: Address,
    pub chain_id: u64,
}

pub fn get_env() -> Result<Env> {
    let profit_recipient: Address = env::var("PROFIT_RECIPIENT_ADDRESS")
        .context("PROFIT_RECIPIENT_ADDRESS")?
        .parse()
        .map_err(|_| anyhow::anyhow!("PROFIT_RECIPIENT_ADDRESS is not a valid 0x address"))?;
    if profit_recipient == Address::ZERO {
        anyhow::bail!("PROFIT_RECIPIENT_ADDRESS cannot be the zero address");
    }
    let chain_id: u64 = env::var("CHAIN_ID")
        .unwrap_or_else(|_| "1".to_string())
        .parse()
        .map_err(|_| anyhow::anyhow!("CHAIN_ID must be a valid u64"))?;
    if chain_id != 1 && chain_id != 11155111 {
        anyhow::bail!("CHAIN_ID must be 1 (mainnet) or 11155111 (sepolia)");
    }
    Ok(Env {
        https_url: env::var("HTTPS_URL").context("HTTPS_URL")?,
        wss_url: env::var("WSS_URL").context("WSS_URL")?,
        bot_address: env::var("BOT_ADDRESS").context("BOT_ADDRESS")?.parse()?,
        private_key: env::var("PRIVATE_KEY").ok(),
        identity_key: env::var("IDENTITY_KEY").unwrap_or_default(),
        telegram_token: env::var("TELEGRAM_TOKEN").ok(),
        telegram_chat_id: env::var("TELEGRAM_CHAT_ID").ok(),
        use_alert: env::var("USE_ALERT").map(|v| v == "true").unwrap_or(false),
        debug: env::var("DEBUG").map(|v| v == "true").unwrap_or(true),
        min_revenue: U256::from_str_radix(&env::var("MIN_REVENUE_WEI")?, 10)?,
        bribe_bps: env::var("BRIBE_BPS")?.parse()?,
        max_gas_price: U256::from_str_radix(&env::var("MAX_GAS_PRICE_GWEI")?, 10)?,
        weth_float: U256::from_str_radix(&env::var("WETH_FLOAT")?, 10)?,
        profit_recipient,
        chain_id,
    })
}
