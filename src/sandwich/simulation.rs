use crate::common::{constants::*, pools::PoolsMap, utils::*};
use alloy::consensus::Transaction as ConsensusTx;
use alloy::network::TransactionResponse;
use alloy::{
    network::Ethereum,
    primitives::{keccak256, Address, Bytes, B256, U256},
    providers::Provider,
    rpc::types::Transaction,
};
use anyhow::Result;
use revm::context_interface::result::ExecutionResult;
use revm::{
    context::{BlockEnv, Context},
    database::{AlloyDB, CacheDB},
    database_interface::{Database, WrapDatabaseAsync},
    context_interface::JournalTr,
    handler::{ExecuteCommitEvm, MainBuilder, MainContext},
    primitives::{KECCAK_EMPTY, TxKind},
    state::AccountInfo,
};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwapDirection { Buy, Sell }

#[derive(Debug, Clone)]
pub struct SwapInfo {
    pub tx_hash: B256,
    pub target_pair: Address,
    pub main_currency: Address,
    pub target_token: Address,
    pub token0_is_main: bool,
    pub direction: SwapDirection,
}

#[derive(Debug, Clone)]
pub struct PendingTxInfo {
    pub tx_hash: B256,
    pub from: Address,
    pub gas_price: U256,
    pub swaps: Vec<SwapInfo>,
}

pub async fn get_pending_tx_info<P: Provider<Ethereum>>(
    provider: &P,
    hash: B256,
    pools: &PoolsMap,
) -> Result<Option<PendingTxInfo>> {
    let Some(tx) = provider.get_transaction_by_hash(hash).await? else { return Ok(None) };
    let Some(to) = tx.to() else { return Ok(None) };

    let params = json!([
        {
            "from": tx.from(),
            "to": to,
            "data": tx.input(),
            "value": format!("{:#x}", tx.value()),
            "gas": format!("{:#x}", tx.gas_limit()),
            "maxFeePerGas": TransactionResponse::max_fee_per_gas(&tx).map(|v| format!("{v:#x}")),
            "maxPriorityFeePerGas": ConsensusTx::max_priority_fee_per_gas(&tx).map(|v| format!("{v:#x}")),
        },
        "pending",
        { "tracer": "callTracer", "tracerConfig": { "withLog": true } }
    ]);

    let trace: serde_json::Value =
        provider.raw_request("debug_traceCall".into(), params).await?;

    let mut logs = Vec::new();
    extract_logs(&trace, &mut logs);
    let swaps = extract_swap_info(&logs, pools, hash);
    if swaps.is_empty() { return Ok(None); }

    Ok(Some(PendingTxInfo {
        tx_hash: hash,
        from: tx.from(),
        gas_price: U256::from(ConsensusTx::max_fee_per_gas(&tx)),
        swaps,
    }))
}

fn extract_logs(frame: &serde_json::Value, out: &mut Vec<serde_json::Value>) {
    if let Some(arr) = frame.get("logs").and_then(|l| l.as_array()) {
        out.extend(arr.iter().cloned());
    }
    if let Some(calls) = frame.get("calls").and_then(|c| c.as_array()) {
        for c in calls { extract_logs(c, out); }
    }
}

fn extract_swap_info(logs: &[serde_json::Value], pools: &PoolsMap, hash: B256) -> Vec<SwapInfo> {
    let mut out = Vec::new();
    for log in logs {
        let Some(topic0) = log["topics"][0].as_str() else { continue };
        let topic = hex_lit(topic0);
        if topic.len() < 4 || topic[..4] != V2_SWAP_EVENT_ID { continue; }

        let Some(pair) = log["address"].as_str().and_then(|a| a.parse::<Address>().ok()) else { continue };
        let Some((t0, t1)) = pools.get(&pair) else { continue };
        let token0_is_main = is_weth(*t0);
        if !token0_is_main && !is_weth(*t1) { continue; }

        let Some(data) = log["data"].as_str().map(hex_lit) else { continue };
        if data.len() < 128 { continue; }
        let a0_in = U256::from_be_slice(&data[0..32]);
        let a1_in = U256::from_be_slice(&data[32..64]);

        let direction = if !a0_in.is_zero() { SwapDirection::Buy } else { SwapDirection::Sell };
        if a1_in.is_zero() && a0_in.is_zero() { continue; }

        out.push(SwapInfo {
            tx_hash: hash,
            target_pair: pair,
            main_currency: WETH,
            target_token: if token0_is_main { *t1 } else { *t0 },
            token0_is_main,
            direction,
        });
    }
    out
}

fn hex_lit(s: &str) -> Vec<u8> { hex::decode(s.trim_start_matches("0x")).unwrap_or_default() }

pub struct Harness<P: Provider<Ethereum> + Clone + 'static> {
    pub provider: P,
    pub executor: Address,
    pub float: U256,
}

impl<P: Provider<Ethereum> + Clone + 'static> Harness<P> {
    async fn prepare(&self, block: u64, swap: &SwapInfo, victim: &Transaction)
        -> Option<CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, P>>>>
    {
        let mut db = CacheDB::new(WrapDatabaseAsync::new(AlloyDB::new(self.provider.clone(), block.into())).unwrap());

        let slot = balance_slot(self.executor, WETH_BALANCE_SLOT);
        db.insert_account_storage(WETH, U256::from_be_bytes(slot.0), self.float).ok()?;
        db.insert_account_info(
            to_h160(self.executor),
            AccountInfo {
                balance: U256::ZERO,
                nonce: 1,
                code_hash: KECCAK_EMPTY,
                code: None,
                account_id: None,
            },
        );
        let _ = db.basic(to_h160(swap.target_pair));
        let _ = db.basic(to_h160(swap.target_token));
        let _ = db.basic(to_h160(WETH));
        let _ = db.basic(to_h160(victim.from()));
        Some(db)
    }

    pub async fn triangle(
        &self,
        block: u64,
        base_fee: U256,
        swap: &SwapInfo,
        victim: &Transaction,
        amount: U256,
    ) -> Option<U256> {
        let db = self.prepare(block, swap, victim).await?;

        // We keep one Evm (and its db) across all three txs so state commits persist.
        let mut ctx = Context::mainnet().with_db(db);
        ctx.cfg.set_spec_and_mainnet_gas_params(revm::primitives::hardfork::SpecId::CANCUN);
        ctx.block = block_env(block, base_fee);
        let mut evm = ctx.build_mainnet();

        let before = evm.ctx.journaled_state.db_mut().basic(to_h160(self.executor)).ok().flatten().map(|a| a.balance).unwrap_or_default();

        // 1. front-run
        let front = encode_instructions(block, &[(
            swap.token0_is_main, swap.target_pair, WETH, amount, U256::ZERO,
        )]);
        let tx_env = make_tx(self.executor, self.executor, front.clone(), 300_000, U256::ZERO);
        let res = evm.transact_commit(tx_env).ok()?;
        if !matches!(res, ExecutionResult::Success { .. }) { return None; }

        // 2. victim
        let mut vtx = make_tx(
            victim.from(),
            victim.to().unwrap(),
            victim.input().clone().into(),
            victim.gas_limit(),
            victim.value(),
        );
        vtx.gas_price = ConsensusTx::max_fee_per_gas(victim);
        let res = evm.transact_commit(vtx).ok()?;
        if !matches!(res, ExecutionResult::Success { .. }) { return None; }

        // 3. back-run
        let tokens = {
            let slot = balance_slot(self.executor, WETH_BALANCE_SLOT);
            evm.ctx.journaled_state.db_mut().storage(to_h160(swap.target_token), slot.into()).ok()?
        };
        let back = encode_instructions(block, &[(
            !swap.token0_is_main, swap.target_pair, swap.target_token, tokens, U256::ZERO,
        )]);
        let tx_env = make_tx(self.executor, self.executor, back.clone(), 300_000, U256::ZERO);
        let res = evm.transact_commit(tx_env).ok()?;
        if !matches!(res, ExecutionResult::Success { .. }) { return None; }

        let after = evm.ctx.journaled_state.db_mut().basic(to_h160(self.executor)).ok().flatten().map(|a| a.balance).unwrap_or_default();
        after.checked_sub(before)
    }
}

pub fn fork_db<P: Provider<Ethereum> + Clone + 'static>(provider: P, block: u64)
    -> CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, P>>> {
    CacheDB::new(WrapDatabaseAsync::new(AlloyDB::new(provider, block.into())).unwrap())
}

fn make_tx(from: Address, to: Address, data: Bytes, gas: u64, value: U256) -> revm::context::TxEnv {
    let mut tx = revm::context::TxEnv::default();
    tx.caller = to_h160(from);
    tx.kind = TxKind::Call(to_h160(to));
    tx.data = data;
    tx.gas_limit = gas;
    tx.value = value;
    tx
}


fn balance_slot(holder: Address, slot: u64) -> B256 {
    let mut buf = [0u8; 64];
    buf[12..32].copy_from_slice(holder.as_slice());
    buf[63] = slot as u8;
    keccak256(buf)
}

fn block_env(block: u64, base_fee: U256) -> BlockEnv {
    BlockEnv {
        number: U256::from(block),
        basefee: base_fee.to::<u64>(),
        gas_limit: 30_000_000,
        ..Default::default()
    }
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
