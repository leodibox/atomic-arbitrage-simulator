use alloy::primitives::Address;
use anyhow::Result;
use std::{collections::HashMap, path::Path};

pub type TokensMap = HashMap<Address, (String, u8)>;

pub fn load_cache() -> Result<TokensMap> {
    let path = Path::new("cache/tokens.csv");
    if !path.exists() {
        return Ok(TokensMap::new());
    }
    let mut map = TokensMap::new();
    for rec in csv::Reader::from_path(path)?.records() {
        let r = rec?;
        map.insert(r[0].parse()?, (r[1].to_string(), r[2].parse()?));
    }
    Ok(map)
}

pub fn save_cache(map: &TokensMap) -> Result<()> {
    std::fs::create_dir_all("cache")?;
    let mut w = csv::Writer::from_path("cache/tokens.csv")?;
    for (addr, (sym, dec)) in map {
        w.write_record([addr.to_string(), sym.clone(), dec.to_string()])?;
    }
    w.flush()?;
    Ok(())
}
