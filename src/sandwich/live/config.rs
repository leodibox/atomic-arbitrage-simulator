use alloy::primitives::{Address, U256};
use anyhow::{bail, Context, Result};
use std::{env, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feed { Txpool, FullPending, Alchemy }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayKind { Flashbots, Titan, Bloxroute }

impl RelayKind {
    fn parse(v: &str) -> Result<Self> {
        match v.trim().to_ascii_lowercase().as_str() {
            "flashbots" => Ok(Self::Flashbots),
            "titan" => Ok(Self::Titan),
            "bloxroute" => Ok(Self::Bloxroute),
            other => bail!("unknown relay kind {other}"),
        }
    }
    pub fn signs_header(self) -> bool { matches!(self, Self::Flashbots | Self::Titan) }
}

#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub name: String,
    pub url: String,
    pub kind: RelayKind,
    pub auth: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SandwichConfig {
    pub https_url: String,
    pub wss_url: String,
    pub executor: Address,
    pub owner_key: String,
    pub identity_key: String,
    pub relays: Vec<RelayConfig>,
    pub routers: Vec<Address>,
    pub max_float_weth: U256,
    pub min_victim_weth: U256,
    pub min_slippage_bps: u64,
    pub min_revenue_weth: U256,
    pub tip_bps: u64,
    pub max_tip_bps: u64,
    pub max_gas_price: U256,
    pub gas_units: u64,
    pub max_blocks: u64,
    pub max_reserve_age: u64,
    pub min_pool_weth: U256,
    pub refresh_blocks: u64,
    pub poll_ms: u64,
    pub feed: Feed,
    pub dry_run: bool,
    pub max_inflight: usize,
    pub unwind_after_blocks: u64,
}

const DEFAULT_ROUTERS: [&str; 3] = [
    "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D",
    "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F",
    "0x03f7724180AA6b939894B5Ca4314783B0b36b329",
];

fn req(k: &str) -> Result<String> {
    env::var(k).with_context(|| format!("missing {k}"))
}

fn addr(k: &str) -> Result<Address> {
    req(k)?.parse().map_err(|_| anyhow::anyhow!("{k} is not an address"))
}

fn num(k: &str, d: u64) -> Result<u64> {
    match env::var(k) {
        Ok(v) if !v.trim().is_empty() => {
            v.trim().parse().with_context(|| format!("{k} must be u64"))
        }
        _ => Ok(d),
    }
}

fn wei(k: &str, d: &str) -> Result<U256> {
    let raw = env::var(k).unwrap_or_else(|_| d.to_string());
    U256::from_str_radix(raw.trim(), 10).with_context(|| format!("{k} must be wei"))
}

fn flag(k: &str, d: bool) -> bool {
    env::var(k).map(|v| v.trim().eq_ignore_ascii_case("true")).unwrap_or(d)
}

fn parse_relays(raw: &str) -> Result<Vec<RelayConfig>> {
    let mut out = Vec::new();
    for entry in raw.split(';').map(str::trim).filter(|v| !v.is_empty()) {
        let parts: Vec<&str> = entry.split('|').map(str::trim).collect();
        if parts.len() < 3 {
            bail!("relay entry must be name|url|kind|auth: {entry}");
        }
        out.push(RelayConfig {
            name: parts[0].to_string(),
            url: parts[1].to_string(),
            kind: RelayKind::parse(parts[2])?,
            auth: parts.get(3).map(|v| v.to_string()).filter(|v| !v.is_empty()),
        });
    }
    Ok(out)
}

impl SandwichConfig {
    pub fn from_env() -> Result<Self> {
        let relays = match env::var("SANDWICH_RELAYS") {
            Ok(v) if !v.trim().is_empty() => parse_relays(&v)?,
            _ => vec![RelayConfig {
                name: "flashbots".into(),
                url: "https://relay.flashbots.net".into(),
                kind: RelayKind::Flashbots,
                auth: None,
            }],
        };

        let routers = match env::var("SANDWICH_ROUTERS") {
            Ok(v) if !v.trim().is_empty() => v
                .split(',')
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(|v| v.parse::<Address>().map_err(|_| anyhow::anyhow!("bad router {v}")))
                .collect::<Result<Vec<_>>>()?,
            _ => DEFAULT_ROUTERS.iter().map(|v| v.parse::<Address>().unwrap()).collect(),
        };

        let feed = match env::var("SANDWICH_FEED").unwrap_or_default().to_ascii_lowercase().as_str() {
            "full" => Feed::FullPending,
            "txpool" => Feed::Txpool,
            _ => Feed::Alchemy,
        };

        let cfg = Self {
            https_url: req("HTTPS_URL")?,
            wss_url: req("WSS_URL")?,
            executor: addr("SANDWICH_EXECUTOR")?,
            owner_key: req("PRIVATE_KEY")?,
            identity_key: req("IDENTITY_KEY")?,
            relays,
            routers,
            max_float_weth: wei("SANDWICH_MAX_FLOAT_WETH", "3000000000000000000")?,
            min_victim_weth: wei("SANDWICH_MIN_VICTIM_WETH", "2000000000000000000")?,
            min_slippage_bps: num("SANDWICH_MIN_SLIPPAGE_BPS", 50)?,
            min_revenue_weth: wei("SANDWICH_MIN_REVENUE_WETH", "100000000000000")?,
            tip_bps: num("SANDWICH_TIP_BPS", 3000)?,
            max_tip_bps: num("SANDWICH_MAX_TIP_BPS", 8000)?,
            max_gas_price: U256::from(num("SANDWICH_MAX_GAS_PRICE_GWEI", 60)?),
            gas_units: num("SANDWICH_GAS_UNITS", 260_000)?,
            max_blocks: num("SANDWICH_MAX_BLOCKS", 2)?,
            max_reserve_age: num("SANDWICH_MAX_RESERVE_AGE_BLOCKS", 1)?,
            min_pool_weth: wei("SANDWICH_MIN_POOL_WETH", "25000000000000000000")?,
            refresh_blocks: num("SANDWICH_REFRESH_BLOCKS", 25)?,
            poll_ms: num("SANDWICH_POLL_MS", 400)?,
            feed,
            dry_run: flag("SANDWICH_DRY_RUN", true),
            max_inflight: num("SANDWICH_MAX_INFLIGHT", 1)? as usize,
            unwind_after_blocks: num("SANDWICH_UNWIND_AFTER_BLOCKS", 3)?,
        };

        if cfg.relays.is_empty() {
            bail!("no relays configured");
        }
        if cfg.max_inflight == 0 {
            bail!("SANDWICH_MAX_INFLIGHT must be >= 1");
        }
        if cfg.tip_bps > cfg.max_tip_bps {
            bail!("SANDWICH_TIP_BPS above SANDWICH_MAX_TIP_BPS");
        }
        Ok(cfg)
    }
}
