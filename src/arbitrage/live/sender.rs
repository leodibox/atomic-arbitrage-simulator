//! The only module that signs or broadcasts an arbitrage transaction.
//!
//! Deliberately dumb: one attempt, no retries, no fee re-pricing. Every
//! precondition is decided by the caller (activation, gate, reservation).
use alloy::{
    consensus::{SignableTransaction, Transaction, TxEnvelope},
    eips::eip2718::Decodable2718,
    eips::Encodable2718,
    network::{Ethereum, NetworkTransactionBuilder, TxSignerSync},
    primitives::{Address, B256, U256},
    providers::Provider,
    rpc::types::{TransactionInput, TransactionRequest},
    signers::local::PrivateKeySigner,
};
use anyhow::{ensure, Context, Result};

#[derive(Clone, Debug)]
pub struct SendRequest {
    pub executor: Address,
    pub calldata: Vec<u8>,
    pub gas_limit: u64,
    pub max_fee_per_gas_wei: U256,
    pub max_priority_fee_per_gas_wei: U256,
    pub nonce: u64,
    pub block: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiptOutcome {
    Landed,
    Reverted,
    StillPending,
}

/// Local sanity only. It cannot prove profitability.
pub fn validate(request: &SendRequest) -> Result<()> {
    ensure!(request.executor != Address::ZERO, "missing executor");
    ensure!(!request.calldata.is_empty(), "empty calldata");
    ensure!(request.calldata.len() >= 4, "calldata lacks a selector");
    ensure!(request.gas_limit > 0, "missing gas limit");
    ensure!(!request.max_fee_per_gas_wei.is_zero(), "missing max fee");
    ensure!(
        request.max_priority_fee_per_gas_wei <= request.max_fee_per_gas_wei,
        "priority fee above the fee cap"
    );
    ensure!(request.block > 0, "missing pinned block");
    Ok(())
}

pub fn build_unsigned(signer: Address, request: &SendRequest) -> Result<TransactionRequest> {
    validate(request)?;
    let mut tx = TransactionRequest::default()
        .from(signer)
        .to(request.executor)
        .input(TransactionInput::both(request.calldata.clone().into()))
        .gas_limit(request.gas_limit)
        .max_fee_per_gas(request.max_fee_per_gas_wei.to::<u128>())
        .max_priority_fee_per_gas(request.max_priority_fee_per_gas_wei.to::<u128>())
        .nonce(request.nonce);
    tx.chain_id = Some(1);
    Ok(tx)
}

/// Exact-sender `eth_call` at the pinned block. A success here is the
/// `exact_sender_simulation_succeeded` evidence, and nothing more.
pub async fn simulate<P: Provider<Ethereum>>(
    provider: &P,
    signer: Address,
    request: &SendRequest,
) -> Result<()> {
    let tx = build_unsigned(signer, request)?;
    provider
        .call(tx)
        .block(request.block.into())
        .await
        .context("exact-sender simulation reverted")?;
    Ok(())
}

/// Sign once and broadcast. Returns the hash; the caller owns the outcome.
pub async fn sign_and_send<P: Provider<Ethereum>>(
    provider: &P,
    signer: &PrivateKeySigner,
    request: &SendRequest,
) -> Result<B256> {
    let tx = build_unsigned(signer.address(), request)?;
    let mut unsigned = tx.build_unsigned().map_err(|error| anyhow::anyhow!("{error}"))?;
    let signature = signer.sign_transaction_sync(&mut unsigned)?;
    let encoded = unsigned.into_signed(signature).encoded_2718();
    let pending = provider
        .send_raw_transaction(encoded.as_ref())
        .await
        .context("broadcast rejected")?;
    Ok(*pending.tx_hash())
}

/// Poll a receipt until the deadline. Never resubmits.
pub async fn await_receipt<P: Provider<Ethereum>>(
    provider: &P,
    hash: B256,
    deadline_secs: u64,
) -> Result<ReceiptOutcome> {
    for _ in 0..deadline_secs.max(1) {
        if let Some(receipt) = provider.get_transaction_receipt(hash).await? {
            return Ok(if receipt.status() {
                ReceiptOutcome::Landed
            } else {
                ReceiptOutcome::Reverted
            });
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    Ok(ReceiptOutcome::StillPending)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signer() -> PrivateKeySigner {
        PrivateKeySigner::from_slice(&[7u8; 32]).unwrap()
    }

    fn request() -> SendRequest {
        SendRequest {
            executor: Address::repeat_byte(0x11),
            calldata: vec![0xde, 0xad, 0xbe, 0xef, 1, 2, 3],
            gas_limit: 420_000,
            max_fee_per_gas_wei: U256::from(20_000_000_000u64),
            max_priority_fee_per_gas_wei: U256::from(1_000_000_000u64),
            nonce: 5,
            block: 21_000_000,
        }
    }

    #[test]
    fn valid_request_passes_local_checks() {
        assert!(validate(&request()).is_ok());
    }

    #[test]
    fn rejects_empty_or_selectorless_calldata() {
        let mut empty = request();
        empty.calldata.clear();
        assert!(validate(&empty).is_err());
        let mut short = request();
        short.calldata = vec![1, 2];
        assert!(validate(&short).is_err());
    }

    #[test]
    fn rejects_tip_above_cap_and_zero_fee() {
        let mut bad_tip = request();
        bad_tip.max_priority_fee_per_gas_wei = U256::from(21_000_000_000u64);
        assert!(validate(&bad_tip).is_err());
        let mut zero_fee = request();
        zero_fee.max_fee_per_gas_wei = U256::ZERO;
        assert!(validate(&zero_fee).is_err());
    }

    #[test]
    fn signing_preserves_every_field() {
        let signer = signer();
        let request = request();
        let tx = build_unsigned(signer.address(), &request).unwrap();
        let mut unsigned = tx.build_unsigned().unwrap();
        let signature = signer.sign_transaction_sync(&mut unsigned).unwrap();
        let encoded = unsigned.into_signed(signature).encoded_2718();
        let mut slice: &[u8] = encoded.as_ref();
        let decoded = TxEnvelope::decode_2718(&mut slice).unwrap();
        assert_eq!(decoded.chain_id(), Some(1));
        assert_eq!(decoded.to().unwrap(), request.executor);
        assert_eq!(decoded.gas_limit(), request.gas_limit);
        assert_eq!(decoded.nonce(), request.nonce);
        assert_eq!(decoded.input(), &request.calldata[..]);
        assert_eq!(decoded.max_priority_fee_per_gas(), Some(1_000_000_000));
    }
}
