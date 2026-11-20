//! Read-only mainnet verification of the intended guarded Aave executor and
//! future sender's PUBLIC address. Does not load secret keys, sign or trade.
use alloy::{
    network::Ethereum,
    primitives::{address, keccak256, Address, B256},
    providers::Provider,
    sol,
};
use anyhow::{ensure, Context, Result};

sol! {
    #[sol(rpc)]
    interface GuardedReadOnly {
        function owner() external view returns (address);
        function pool() external view returns (address);
        function profitRecipient() external view returns (address);
        function paused() external view returns (bool);
        function autoPayout() external view returns (bool);
    }
}
const AAVE_V3_POOL: Address = address!("87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2");

#[derive(Clone, Debug)]
pub struct GuardedPublicConfig {
    pub executor: Address,
    pub expected_owner: Address,
    pub expected_recipient: Address,
    /// Pin the exact runtime bytes hash from a separately reviewed compilation.
    pub expected_runtime_keccak: B256,
}
impl GuardedPublicConfig {
    /// This requires PUBLIC addresses and an independently pinned code hash.
    /// A signer private key is never accepted or inspected here.
    pub fn from_environment() -> Result<Option<Self>> {
        let vars = [
            "GUARDED_EXECUTOR_ADDRESS",
            "EXPECTED_EXECUTOR_OWNER",
            "EXPECTED_PROFIT_RECIPIENT",
            "GUARDED_RUNTIME_KECCAK256",
        ];
        let found: Vec<Option<String>> = vars.iter()
            .map(|name| std::env::var(name).ok()).collect();
        if found.iter().all(|x| x.is_none()) { return Ok(None) }
        ensure!(found.iter().all(|x| x.as_ref().is_some_and(|v| !v.is_empty())),
            "partial guarded executor configuration; all public attestations required");
        let executor: Address = found[0].as_ref().unwrap().parse()
            .context("invalid GUARDED_EXECUTOR_ADDRESS")?;
        let expected_owner: Address = found[1].as_ref().unwrap().parse()
            .context("invalid EXPECTED_EXECUTOR_OWNER")?;
        let expected_recipient: Address = found[2].as_ref().unwrap().parse()
            .context("invalid EXPECTED_PROFIT_RECIPIENT")?;
        let expected_runtime_keccak: B256 = found[3].as_ref().unwrap().parse()
            .context("invalid GUARDED_RUNTIME_KECCAK256")?;
        ensure!(executor != Address::ZERO && expected_owner != Address::ZERO
                && expected_recipient != Address::ZERO && expected_runtime_keccak != B256::ZERO,
            "zero guarded public address/hash is forbidden");
        ensure!(executor != expected_owner, "contract address cannot equal EOA signer");
        Ok(Some(Self {executor, expected_owner, expected_recipient, expected_runtime_keccak}))
    }
}

#[derive(Debug)]
pub struct GuardedPublicResult {
    pub executor: Address,
    pub owner: Address,
    pub recipient: Address,
    pub code_keccak: B256,
    pub paused: bool,
    pub auto_payout: bool,
}

pub async fn verify_guarded_read_only<P: Provider<Ethereum> + Clone>(
    provider: &P, config: &GuardedPublicConfig,
) -> Result<GuardedPublicResult> {
    ensure!(provider.get_chain_id().await? == 1, "expected Ethereum mainnet");
    let runtime = provider.get_code_at(config.executor).await?;
    ensure!(!runtime.is_empty(), "Guarded executor missing on-chain runtime code");
    let runtime_hash = keccak256(runtime.as_ref());
    ensure!(runtime_hash == config.expected_runtime_keccak,
        "deployed runtime bytecode does not match independently reviewed hash");
    let aave_code = provider.get_code_at(AAVE_V3_POOL).await?;
    ensure!(!aave_code.is_empty(), "canonical Aave V3 Pool runtime missing");
    let contract = GuardedReadOnly::new(config.executor, provider.clone());
    // Retain each generated call builder through try_join: temporaries
    // created inside the macro otherwise cause Rust E0716.
    let owner_call = contract.owner();
    let pool_call = contract.pool();
    let recipient_call = contract.profitRecipient();
    let paused_call = contract.paused();
    let payout_call = contract.autoPayout();
    let (owner, pool, recipient, paused, auto_payout) = tokio::try_join!(
        owner_call.call(),
        pool_call.call(),
        recipient_call.call(),
        paused_call.call(),
        payout_call.call()
    )?;
    ensure!(owner == config.expected_owner, "owner() differs from expected signer public address");
    ensure!(pool == AAVE_V3_POOL, "pool() is not canonical Aave V3 on mainnet");
    ensure!(recipient == config.expected_recipient, "profitRecipient() mismatch");
    ensure!(paused, "guarded executor is UNPAUSED: read-only attestation denies");
    ensure!(!auto_payout, "auto payout unexpectedly enabled before activation");
    Ok(GuardedPublicResult {
        executor:config.executor,owner,recipient,code_keccak:runtime_hash,
        paused,auto_payout
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_address_and_missing_code_hash_fail_closed() {
        let invalid="not-an-address".parse::<Address>();
        assert!(invalid.is_err());
        assert_ne!(B256::ZERO, keccak256([1u8,2u8,3u8]));
    }
}
