use super::config::{RelayConfig, RelayKind};
use alloy::primitives::{keccak256, Bytes};
use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct BundleRequest {
    pub txs: Vec<Bytes>,
    pub block: u64,
    pub reverting: Vec<String>,
    pub replacement_uuid: String,
}

#[derive(Debug, Clone)]
pub struct RelayOutcome {
    pub relay: String,
    pub ok: bool,
    pub detail: String,
}

pub struct RelayClient {
    http: reqwest::Client,
    relays: Arc<Vec<RelayConfig>>,
    identity: PrivateKeySigner,
}

pub fn new_uuid() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let mut buf = [0u8; 32];
    buf[..16].copy_from_slice(&nanos.to_be_bytes());
    buf[16..24].copy_from_slice(&(nanos as u64 ^ 0x9e37_79b9_7f4a_7c15).to_be_bytes());
    let h = keccak256(buf);
    format!("0x{}", hex::encode(&h.as_slice()[..16]))
}

impl RelayClient {
    pub fn new(relays: Arc<Vec<RelayConfig>>, identity: PrivateKeySigner) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(4))
                .build()
                .unwrap_or_default(),
            relays,
            identity,
        }
    }

    async fn headers(
        &self,
        kind: RelayKind,
        body: &[u8],
        auth: Option<&str>,
    ) -> Result<Vec<(String, String)>> {
        let mut out = vec![("Content-Type".to_string(), "application/json".to_string())];
        if kind.signs_header() {
            let sig = self.identity.sign_hash(&keccak256(body)).await?;
            out.push((
                "X-Flashbots-Signature".to_string(),
                format!(
                    "{:#x}:0x{}",
                    self.identity.address(),
                    hex::encode(sig.as_bytes())
                ),
            ));
        }
        if let Some(key) = auth {
            out.push(("Authorization".to_string(), key.to_string()));
        }
        Ok(out)
    }

    async fn call(&self, relay: &RelayConfig, method: &str, params: Value) -> Result<Value> {
        let body =
            json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }).to_string();
        let mut req = self.http.post(&relay.url);
        for (k, v) in self
            .headers(relay.kind, body.as_bytes(), relay.auth.as_deref())
            .await?
        {
            req = req.header(k, v);
        }
        let resp = req.body(body).send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!(
                "http {status}: {}",
                text.chars().take(200).collect::<String>()
            );
        }
        let value: Value = serde_json::from_str(&text)?;
        if let Some(err) = value.get("error") {
            bail!("{err}");
        }
        Ok(value)
    }

    pub async fn send_bundle(&self, bundle: &BundleRequest) -> Vec<RelayOutcome> {
        let txs: Vec<String> = bundle
            .txs
            .iter()
            .map(|t| format!("0x{}", hex::encode(t)))
            .collect();
        let params = json!([{
            "txs": txs,
            "blockNumber": format!("{:#x}", bundle.block),
            "minTimestamp": 0,
            "maxTimestamp": u64::MAX,
            "revertingTxHashes": bundle.reverting,
            "replacementUuid": bundle.replacement_uuid,
        }]);

        let mut futs = Vec::new();
        for relay in self.relays.iter() {
            let relay = relay.clone();
            let params = params.clone();
            futs.push(async move {
                match self.call(&relay, "eth_sendBundle", params).await {
                    Ok(v) => {
                        let h = v
                            .get("result")
                            .and_then(|r| r.get("bundleHash"))
                            .and_then(|h| h.as_str())
                            .unwrap_or("accepted");
                        RelayOutcome {
                            relay: relay.name,
                            ok: true,
                            detail: h.to_string(),
                        }
                    }
                    Err(e) => RelayOutcome {
                        relay: relay.name,
                        ok: false,
                        detail: e.to_string(),
                    },
                }
            });
        }
        futures::future::join_all(futs).await
    }

    pub async fn cancel(&self, uuid: &str, block: u64) -> Vec<RelayOutcome> {
        let mut out = Vec::new();
        for relay in self.relays.iter() {
            let params = json!([{
                "txs": Vec::<String>::new(),
                "blockNumber": format!("{block:#x}"),
                "replacementUuid": uuid,
            }]);
            out.push(match self.call(relay, "eth_sendBundle", params).await {
                Ok(_) => RelayOutcome {
                    relay: relay.name.clone(),
                    ok: true,
                    detail: "cancelled".into(),
                },
                Err(e) => RelayOutcome {
                    relay: relay.name.clone(),
                    ok: false,
                    detail: e.to_string(),
                },
            });
        }
        out
    }
}
