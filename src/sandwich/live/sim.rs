use crate::common::constants::WETH;
use crate::common::utils::to_h160;
use super::execution_helpers::build_execute_calldata;
use alloy::consensus::{Transaction as ConsensusTx, TxEnvelope};
use alloy::network::{Ethereum, TransactionResponse};
use alloy::primitives::{keccak256, Address, B256, Bytes, U256};
use alloy::providers::Provider;
use alloy::rpc::types::Transaction;
use revm::context::result::ExecutionResult;
use revm::database::{AlloyDB, CacheDB};
use revm::database_interface::Database;
use revm::database_interface::WrapDatabaseAsync;
use revm::primitives::KECCAK_EMPTY;
use revm::{
    context::{BlockEnv, Context},
    context_interface::JournalTr,
    handler::{ExecuteCommitEvm, MainBuilder, MainContext},
    primitives::TxKind,
    state::AccountInfo,
};

pub struct Confirm {
    pub owner: Address,
    pub executor: Address,
    pub weth_float: U256,
    pub token_float: U256,
}

fn slot(holder: Address, index: u64) -> B256 {
    let mut buf = [0u8; 64];
    buf[12..32].copy_from_slice(holder.as_slice());
    buf[63] = index as u8;
    keccak256(buf)
}

fn tx_env(from: Address, to: Address, data: Bytes, gas: u64, gas_price: u128) -> revm::context::TxEnv {
    let mut tx = revm::context::TxEnv::default();
    tx.caller = to_h160(from);
    tx.kind = TxKind::Call(to_h160(to));
    tx.data = data;
    tx.gas_limit = gas;
    tx.gas_price = gas_price;
    tx
}

pub async fn confirm<P: Provider<Ethereum> + Clone + 'static>(
    provider: P,
    block: u64,
    base_fee: U256,
    min_profit: U256,
    victim: &Transaction<TxEnvelope>,
    pair: Address,
    front_dir: bool,
    front_amount: U256,
    token_in: Address,
    token_out: Address,
    c: &Confirm,
) -> Option<U256> {
    let mut db = CacheDB::new(WrapDatabaseAsync::new(AlloyDB::new(provider, block.into())).unwrap());

    db.insert_account_storage(WETH, U256::from_be_bytes(slot(c.executor, 3).0), c.weth_float).ok()?;
    db.insert_account_storage(token_out, U256::from_be_bytes(slot(c.executor, 3).0), c.token_float).ok()?;
    let (code_hash, code) = match db.basic(to_h160(c.executor)).ok().flatten() {
        Some(a) => (a.code_hash, a.code),
        None => (KECCAK_EMPTY, None),
    };
    if code.is_none() {
        return None;
    }
    db.insert_account_info(
        to_h160(c.executor),
        AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash,
            code,
            account_id: None,
        },
    );
    let owner_live = db.basic(to_h160(c.owner)).ok().flatten();
    db.insert_account_info(
        to_h160(c.owner),
        AccountInfo {
            balance: U256::from(100_000_000_000_000_000_000u128),
            nonce: owner_live.as_ref().map(|a| a.nonce).unwrap_or(0),
            code_hash: owner_live.as_ref().map(|a| a.code_hash).unwrap_or(KECCAK_EMPTY),
            code: owner_live.and_then(|a| a.code),
            account_id: None,
        },
    );
    let _ = db.basic(to_h160(pair));
    let _ = db.basic(to_h160(token_in));
    let _ = db.basic(to_h160(token_out));
    let _ = db.basic(to_h160(WETH));

    let mut ctx = Context::mainnet().with_db(db);
    ctx.cfg.set_spec_and_mainnet_gas_params(revm::primitives::hardfork::SpecId::CANCUN);
    ctx.block = BlockEnv {
        number: U256::from(block),
        basefee: base_fee.to::<u64>(),
        gas_limit: 30_000_000,
        ..Default::default()
    };
    let mut evm = ctx.build_mainnet();

    let before = evm
        .ctx
        .journaled_state
        .db_mut()
        .storage(to_h160(WETH), slot(c.owner, 3).into())
        .ok()
        .unwrap_or_default();

    let mut data = Vec::with_capacity(36);
    data.extend_from_slice(&[0x70u8, 0xa0, 0x82, 0x31]);
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(c.executor.as_slice());
    let pre = match evm
        .transact_commit(tx_env(c.owner, token_out, Bytes::from(data), 30_000, base_fee.to::<u128>()))
        .ok()?
    {
        ExecutionResult::Success { output, .. } => {
            let d = output.data().as_ref();
            if d.len() < 32 {
                return None;
            }
            U256::from_be_slice(&d[..32])
        }
        _ => return None,
    };
    let front = build_execute_calldata(block, U256::ZERO, &[(front_dir, pair, token_in, front_amount)]);
    match evm.transact_commit(tx_env(c.owner, c.executor, front, 220_000, base_fee.to::<u128>())).ok()? {
        ExecutionResult::Success { .. } => {}
        _ => return None,
    }

    let mut vtx = tx_env(victim.from(), victim.to()?, victim.input().clone(), victim.gas_limit(), ConsensusTx::max_fee_per_gas(victim));
    vtx.gas_price = ConsensusTx::max_fee_per_gas(victim);
    vtx.value = victim.value();
    match evm.transact_commit(vtx).ok()? {
        ExecutionResult::Success { .. } => {}
        _ => return None,
    }

    let mut data = Vec::with_capacity(36);
    data.extend_from_slice(&[0x70u8, 0xa0, 0x82, 0x31]);
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(c.executor.as_slice());
    let post = match evm
        .transact_commit(tx_env(c.owner, token_out, Bytes::from(data), 30_000, base_fee.to::<u128>()))
        .ok()?
    {
        ExecutionResult::Success { output, .. } => {
            let d = output.data().as_ref();
            if d.len() < 32 {
                return None;
            }
            U256::from_be_slice(&d[..32])
        }
        _ => return None,
    };
    let held = post.saturating_sub(pre);
    if held.is_zero() {
        return None;
    }

    let back = build_execute_calldata(block, min_profit, &[(!front_dir, pair, token_out, held)]);
    match evm.transact_commit(tx_env(c.owner, c.executor, back, 220_000, base_fee.to::<u128>())).ok()? {
        ExecutionResult::Success { .. } => {}
        _ => return None,
    }

    let after = evm
        .ctx
        .journaled_state
        .db_mut()
        .storage(to_h160(WETH), slot(c.owner, 3).into())
        .ok()
        .unwrap_or_default();
    after.checked_sub(before)
}
