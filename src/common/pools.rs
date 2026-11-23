use alloy::{
    network::Ethereum,
    primitives::{Address, B256},
    providers::Provider,
    rpc::types::Filter,
};
use anyhow::Result;
use std::{collections::HashMap, path::Path};

pub type PoolsMap = HashMap<Address, (Address, Address)>;

pub async fn load_all_pools<P: Provider<Ethereum>>(provider: &P) -> Result<PoolsMap> {
    // Free RPC plans commonly reject large historical eth_getLogs ranges.
    // Hosted simulation deployments can disable this scan and use an
    // allowlisted or prebuilt cache instead.
    if std::env::var("POOL_SCAN_ENABLED")
        .map(|v| v.eq_ignore_ascii_case("false"))
        .unwrap_or(false)
    {
        tracing::warn!("historical pool scan disabled by POOL_SCAN_ENABLED=false");
        return Ok(PoolsMap::new());
    }

    let path = Path::new("cache/pools.csv");
    if path.exists() {
        return read_cache(path);
    }

    let head = provider.get_block_number().await?;
    let mut map = PoolsMap::new();
    let mut start = crate::common::constants::POOL_SCAN_START_BLOCK;

    while start <= head {
        let end = (start + crate::common::constants::POOL_SCAN_BATCH).min(head);
        let filter = Filter::new()
            .address(crate::common::constants::FACTORY_V2)
            .event_signature(B256::from_slice(&hex_lit(
                crate::common::constants::PAIR_CREATED_TOPIC,
            )))
            .from_block(start)
            .to_block(end);

        for log in provider.get_logs(&filter).await? {
            if log.topics().len() < 3 {
                continue;
            }
            let t0 = Address::from_slice(&log.topics()[1].as_slice()[12..]);
            let t1 = Address::from_slice(&log.topics()[2].as_slice()[12..]);
            let data = log.data().data.as_ref();
            if data.len() < 32 {
                continue;
            }
            let pair = Address::from_slice(&data[12..32]);
            map.insert(pair, (t0, t1));
        }
        start = end + 1;
    }
    write_cache(path, &map)?;
    Ok(map)
}

fn hex_lit(s: &str) -> Vec<u8> {
    hex::decode(s.trim_start_matches("0x")).unwrap()
}

fn read_cache(path: &Path) -> Result<PoolsMap> {
    let mut map = PoolsMap::new();
    for rec in csv::Reader::from_path(path)?.records() {
        let r = rec?;
        map.insert(r[0].parse()?, (r[1].parse()?, r[2].parse()?));
    }
    Ok(map)
}

fn write_cache(path: &Path, map: &PoolsMap) -> Result<()> {
    std::fs::create_dir_all("cache")?;
    let mut w = csv::Writer::from_path(path)?;
    for (pair, (t0, t1)) in map {
        w.write_record([pair.to_string(), t0.to_string(), t1.to_string()])?;
    }
    w.flush()?;
    Ok(())
}
