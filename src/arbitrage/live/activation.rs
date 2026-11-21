//! Fail-closed, READ-ONLY verification that a specific candidate may execute.
//!
//! The guarded executor ships paused, with zero loan caps, no approved pairs
//! and no operators. A previous attestation proves identity; this proves
//! *permission for this exact route*. Nothing here signs or sends.
use alloy::{
    network::Ethereum,
    primitives::{keccak256, Address, B256, U256},
    providers::Provider,
    sol,
};
use anyhow::{ensure, Context, Result};

sol! {
    #[sol(rpc)]
    interface GuardedPermission {
        function owner() external view returns (address);
        function pool() external view returns (address);
        function profitRecipient() external view returns (address);
        function paused() external view returns (bool);
        function autoPayout() external view returns (bool);
        function maxLoanRaw(address asset) external view returns (uint256);
        function approvedPair(address pair) external view returns (bool);
        function approvedOperator(address operator) external view returns (bool);
    }
}

#[derive(Clone, Debug)]
pub struct ActivationRequest {
    pub executor: Address,
    pub signer: Address,
    pub asset: Address,
    pub amount: U256,
    /// Ordered hop pairs, already validated as a 2-3 hop route.
    pub pairs: Vec<Address>,
    pub expected_owner: Address,
    pub expected_recipient: Address,
    pub expected_pool: Address,
    pub expected_runtime_keccak: B256,
}

#[derive(Clone, Debug)]
pub struct ActivationReport {
    pub cap_raw: U256,
    pub owner_is_signer: bool,
    pub operator_approved: bool,
    pub auto_payout: bool,
    pub pairs_checked: usize,
}

/// Route shape rules mirrored from the contract, checked before any RPC.
pub fn validate_pairs(pairs: &[Address]) -> Result<()> {
    ensure!(
        (2..=3).contains(&pairs.len()),
        "route must contain 2 or 3 pairs"
    );
    ensure!(pairs.iter().all(|pair| *pair != Address::ZERO), "zero pair");
    for (index, pair) in pairs.iter().enumerate() {
        ensure!(!pairs[..index].contains(pair), "duplicate pair in route");
    }
    Ok(())
}

pub fn cap_allows(cap_raw: U256, amount: U256) -> Result<()> {
    ensure!(!amount.is_zero(), "zero loan amount");
    ensure!(
        cap_raw >= amount,
        "loan cap {cap_raw} below requested {amount}"
    );
    Ok(())
}

/// Every check fails closed. A partial answer is never permission.
pub async fn verify_permission<P: Provider<Ethereum> + Clone>(
    provider: &P,
    request: &ActivationRequest,
) -> Result<ActivationReport> {
    ensure!(
        provider.get_chain_id().await? == 1,
        "activation verification is Ethereum mainnet only"
    );
    ensure!(
        request.executor != Address::ZERO
            && request.signer != Address::ZERO
            && request.asset != Address::ZERO,
        "activation request has a zero address"
    );
    ensure!(
        request.executor != request.signer,
        "executor cannot equal the signer"
    );
    validate_pairs(&request.pairs)?;

    let runtime = provider.get_code_at(request.executor).await?;
    ensure!(!runtime.is_empty(), "executor has no runtime code");
    ensure!(
        keccak256(runtime.as_ref()) == request.expected_runtime_keccak,
        "executor runtime bytecode does not match the pinned hash"
    );

    let contract = GuardedPermission::new(request.executor, provider.clone());
    let owner = contract.owner().call().await?;
    let pool = contract.pool().call().await?;
    let recipient = contract.profitRecipient().call().await?;
    let paused = contract.paused().call().await?;
    let auto_payout = contract.autoPayout().call().await?;

    ensure!(owner == request.expected_owner, "owner() mismatch");
    ensure!(pool == request.expected_pool, "pool() is not canonical Aave");
    ensure!(
        recipient == request.expected_recipient,
        "profitRecipient() mismatch"
    );
    ensure!(!paused, "executor is still PAUSED");
    ensure!(!recipient.is_zero(), "profit recipient is zero");

    let cap_raw = contract.maxLoanRaw(request.asset).call().await?;
    cap_allows(cap_raw, request.amount)?;

    let owner_is_signer = owner == request.signer;
    let operator_approved = if owner_is_signer {
        true
    } else {
        contract
            .approvedOperator(request.signer)
            .call()
            .await
            .context("operator approval read failed")?
    };
    ensure!(
        owner_is_signer || operator_approved,
        "signer is neither the owner nor an approved operator"
    );

    for pair in &request.pairs {
        ensure!(
            contract.approvedPair(*pair).call().await?,
            "pair {pair} is not approved"
        );
    }

    Ok(ActivationReport {
        cap_raw,
        owner_is_signer,
        operator_approved,
        auto_payout,
        pairs_checked: request.pairs.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(byte: u8) -> Address {
        Address::repeat_byte(byte)
    }

    #[test]
    fn accepts_two_and_three_hop_routes() {
        assert!(validate_pairs(&[pair(1), pair(2)]).is_ok());
        assert!(validate_pairs(&[pair(1), pair(2), pair(3)]).is_ok());
    }

    #[test]
    fn rejects_wrong_length_zero_and_duplicate_pairs() {
        assert!(validate_pairs(&[pair(1)]).is_err());
        assert!(validate_pairs(&[pair(1), pair(2), pair(3), pair(4)]).is_err());
        assert!(validate_pairs(&[pair(1), Address::ZERO]).is_err());
        assert!(validate_pairs(&[pair(1), pair(1)]).is_err());
        assert!(validate_pairs(&[pair(1), pair(2), pair(1)]).is_err());
    }

    #[test]
    fn cap_must_cover_the_loan_and_be_nonzero() {
        assert!(cap_allows(U256::from(100u64), U256::from(100u64)).is_ok());
        assert!(cap_allows(U256::from(99u64), U256::from(100u64)).is_err());
        assert!(cap_allows(U256::ZERO, U256::ZERO).is_err());
    }

    #[test]
    fn zero_cap_disables_the_asset() {
        assert!(cap_allows(U256::ZERO, U256::from(1u64)).is_err());
    }

    #[test]
    fn report_keeps_the_evidence_it_observed() {
        let report = ActivationReport {
            cap_raw: U256::from(5u64),
            owner_is_signer: false,
            operator_approved: true,
            auto_payout: false,
            pairs_checked: 2,
        };
        assert_eq!(report.pairs_checked, 2);
        assert!(!report.auto_payout);
        assert!(report.operator_approved);
    }

    #[test]
    fn exact_cap_boundary_is_allowed() {
        assert!(cap_allows(U256::from(7u64), U256::from(7u64)).is_ok());
    }
}
