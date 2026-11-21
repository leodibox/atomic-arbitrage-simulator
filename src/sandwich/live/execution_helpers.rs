use alloy::consensus::SignableTransaction;
use alloy::eips::Encodable2718;
use alloy::network::{Ethereum, NetworkTransactionBuilder, TxSignerSync};
use alloy::primitives::{keccak256, Address, Bytes, U256};
use alloy::providers::Provider;
use alloy::rpc::types::{TransactionInput, TransactionRequest};
use alloy::signers::local::PrivateKeySigner;
use anyhow::Result;

pub fn selector() -> [u8; 4] {
    let h = keccak256("execute(uint256)");
    [h[0], h[1], h[2], h[3]]
}

fn push_steps(out: &mut Vec<u8>, steps: &[(bool, Address, Address, U256)]) {
    for (dir, pair, token_in, amount) in steps {
        out.push(*dir as u8);
        out.extend_from_slice(pair.as_slice());
        out.extend_from_slice(token_in.as_slice());
        out.extend_from_slice(&amount.to_be_bytes::<32>());
        out.extend_from_slice(&U256::ZERO.to_be_bytes::<32>());
    }
}

pub fn build_execute_calldata(
    block: u64,
    min_profit: U256,
    steps: &[(bool, Address, Address, U256)],
) -> Bytes {
    let mut out = Vec::with_capacity(44 + steps.len() * 105);
    out.extend_from_slice(&selector());
    out.extend_from_slice(&min_profit.to_be_bytes::<32>());
    out.extend_from_slice(&block.to_be_bytes());
    push_steps(&mut out, steps);
    Bytes::from(out)
}

pub fn build_step_calldata(block: u64, steps: &[(bool, Address, Address, U256)]) -> Bytes {
    let mut out = Vec::with_capacity(8 + steps.len() * 105);
    out.extend_from_slice(&block.to_be_bytes());
    push_steps(&mut out, steps);
    Bytes::from(out)
}

pub fn sign_execute(
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
        .input(TransactionInput::both(calldata))
        .gas_limit(gas_limit)
        .max_fee_per_gas(max_fee.to::<u128>())
        .max_priority_fee_per_gas(tip.to::<u128>())
        .nonce(nonce);
    req.chain_id = Some(1);
    let mut unsigned = req.build_unsigned().map_err(|e| anyhow::anyhow!("{e}"))?;
    let signature = signer.sign_transaction_sync(&mut unsigned)?;
    Ok(unsigned.into_signed(signature).encoded_2718().into())
}

pub async fn erc20_balance<P: Provider<Ethereum>>(
    provider: &P,
    token: Address,
    owner: Address,
) -> Result<U256> {
    let mut data = vec![0x70u8, 0xa0, 0x82, 0x31];
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(owner.as_slice());
    let res = provider
        .call(TransactionRequest::default().to(token).input(TransactionInput::both(Bytes::from(data))))
        .await?;
    if res.len() < 32 {
        return Ok(U256::ZERO);
    }
    Ok(U256::from_be_slice(&res[..32]))
}
