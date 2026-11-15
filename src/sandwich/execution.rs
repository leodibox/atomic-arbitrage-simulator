use crate::common::constants::COINBASE;
use alloy::consensus::SignableTransaction;
use alloy::eips::Encodable2718;
use alloy::network::{Ethereum, NetworkTransactionBuilder, TxSignerSync};
use alloy::primitives::{Address, Bytes, U256};
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use anyhow::Result;
use serde_json::json;

pub struct Bundle {
    pub block: u64,
    pub txs: Vec<Bytes>,
    pub gas_used: u64,
    pub bribe: U256,
}

pub fn encode_instructions(block: u64, steps: &[(bool, Address, Address, U256, U256)]) -> Bytes {
    let mut out = Vec::with_capacity(8 + steps.len() * 105);
    out.extend_from_slice(&block.to_be_bytes());
    for (zero_for_one, pair, token_in, amount_in, amount_out) in steps {
        out.push(*zero_for_one as u8);
        out.extend_from_slice(pair.as_slice());
        out.extend_from_slice(token_in.as_slice());
        out.extend_from_slice(&amount_in.to_be_bytes::<32>());
        out.extend_from_slice(&amount_out.to_be_bytes::<32>());
    }
    Bytes::from(out)
}

pub fn sign_executor_tx(
    signer: &PrivateKeySigner,
    executor: Address,
    calldata: Bytes,
    gas_limit: u64,
    max_fee: U256,
    tip: U256,
    nonce: u64,
) -> Result<Bytes> {
    let mut req = TransactionRequest::default()
        .from(signer.address())
        .to(executor)
        .input(calldata.into())
        .gas_limit(gas_limit)
        .max_fee_per_gas(max_fee.to::<u128>())
        .max_priority_fee_per_gas(tip.to::<u128>())
        .nonce(nonce);
    req.chain_id = Some(1);
    let mut unsigned = req.build_unsigned().map_err(|e| anyhow::anyhow!("{e}"))?;
    let signature = signer.sign_transaction_sync(&mut unsigned)?;
    let signed = unsigned.into_signed(signature);
    Ok(signed.encoded_2718().into())
}

pub async fn send_bundle<P: Provider<Ethereum>>(
    provider: &P,
    bundle: &Bundle,
) -> Result<serde_json::Value> {
    let params = json!([
        {
            "txs": bundle.txs.iter().map(|t| format!("0x{}", hex::encode(t))).collect::<Vec<_>>(),
            "blockNumber": format!("{:#x}", bundle.block),
            "minTimestamp": 0,
            "maxTimestamp": u64::MAX,
        }
    ]);
    Ok(provider.raw_request("eth_sendBundle".into(), params).await?)
}

pub async fn simulate_bundle<P: Provider<Ethereum>>(
    provider: &P,
    bundle: &Bundle,
) -> Result<serde_json::Value> {
    let params = json!([
        {
            "txs": bundle.txs.iter().map(|t| format!("0x{}", hex::encode(t))).collect::<Vec<_>>(),
            "blockNumber": format!("{:#x}", bundle.block),
        }
    ]);
    Ok(provider.raw_request("eth_callBundle".into(), params).await?)
}

pub fn coinbase_payment(bribe: U256) -> (Address, U256) { (COINBASE, bribe) }
