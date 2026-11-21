use crate::common::constants::WETH;
use crate::common::utils::to_h160;
use crate::sandwich::simulation::encode_instructions;
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

fn tx_env(from: Address, to: Address, data: Bytes, gas: u64) -> revm::context::TxEnv {
    let mut tx = revm::context::TxEnv::default();
    tx.caller = to_h160(from);
    tx.kind = TxKind::Call(to_h160(to));
    tx.data = data;
    tx.gas_limit = gas;
    tx
}

pub async fn confirm<P: Provider<Ethereum> + Clone + 'static>(
    provider: P,
    block: u64,
    base_fee: U256,
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
    db.insert_account_info(
        to_h160(c.executor),
        AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash: KECCAK_EMPTY,
            code: None,
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
        .basic(to_h160(c.executor))
        .ok()
        .flatten()
        .map(|a| a.balance)
        .unwrap_or_default();

    let front = encode_instructions(block, &[(front_dir, pair, token_in, front_amount, U256::ZERO)]);
    match evm.transact_commit(tx_env(c.executor, c.executor, front, 220_000)).ok()? {
        ExecutionResult::Success { .. } => {}
        _ => return None,
    }

    let mut vtx = tx_env(victim.from(), victim.to()?, victim.input().clone(), victim.gas_limit());
    vtx.gas_price = ConsensusTx::max_fee_per_gas(victim);
    vtx.value = victim.value();
    match evm.transact_commit(vtx).ok()? {
        ExecutionResult::Success { .. } => {}
        _ => return None,
    }

    let held = evm
        .ctx
        .journaled_state
        .db_mut()
        .storage(to_h160(token_out), slot(c.executor, 3).into())
        .ok()?;
    if held.is_zero() {
        return None;
    }

    let back = encode_instructions(block, &[(!front_dir, pair, token_out, held, U256::ZERO)]);
    match evm.transact_commit(tx_env(c.executor, c.executor, back, 220_000)).ok()? {
        ExecutionResult::Success { .. } => {}
        _ => return None,
    }

    let after = evm
        .ctx
        .journaled_state
        .db_mut()
        .basic(to_h160(c.executor))
        .ok()
        .flatten()
        .map(|a| a.balance)
        .unwrap_or_default();
    after.checked_sub(before)
}
