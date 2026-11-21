//! Inert discovery-to-executor candidate evidence. NEVER signs or broadcasts.
//! A successful construction is NOT permission to execute a transaction.
use super::preflight::PreparedV2FlashLoan;
use alloy::primitives::{keccak256, Address, B256, U256};
use anyhow::{ensure, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadOnlyCandidate {
    pub chain_id: u64,
    pub block: u64,
    pub calldata_hash: B256,
    pub asset: Address,
    pub loan_amount: U256,
    pub pair_count: usize,
    pub estimated_net_after_modeled_costs: U256,
}

impl ReadOnlyCandidate {
    /// Only consumed after prepare_v2_flash_loan AND modeled-positive-net
    /// checks have succeeded on the SAME pinned block.
    /// This does not attest sender, nonce, deployed bytecode, eth_call, gas
    /// limits in USD, durable reservation, or signer availability.
    pub fn from_preflight(
        chain_id: u64,
        checked_block: u64,
        prepared: &PreparedV2FlashLoan,
        net_after_modeled_costs: U256,
    ) -> Result<Self> {
        ensure!(chain_id == 1, "read-only handoff is Ethereum mainnet only");
        ensure!(checked_block > 0 && prepared.quoted_at_block == checked_block,
            "candidate state and preflight block mismatch");
        ensure!(prepared.asset != Address::ZERO && !prepared.amount.is_zero(),
            "missing loan asset or amount");
        ensure!(!net_after_modeled_costs.is_zero(),
            "no modeled net profit; do not create candidate");
        ensure!(prepared.pair_path.len() % 21 == 0,
            "invalid legacy V2 path encoding");
        let pair_count = prepared.pair_path.len() / 21;
        ensure!((2..=3).contains(&pair_count),
            "unsupported V2 route length");
        ensure!(!prepared.calldata.is_empty(),
            "missing preflight calldata");
        ensure!(!prepared.min_profit_onchain.is_zero(),
            "missing positive onchain minimum profit");
        // A hash is a correlation identifier, not proof that a transaction
        // using these bytes would succeed.
        Ok(Self {
            chain_id,
            block: checked_block,
            calldata_hash: keccak256(&prepared.calldata),
            asset: prepared.asset,
            loan_amount: prepared.amount,
            pair_count,
            estimated_net_after_modeled_costs: net_after_modeled_costs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    fn prepared() -> PreparedV2FlashLoan {
        PreparedV2FlashLoan {
            asset: address!("0000000000000000000000000000000000000001"),
            amount: U256::from(1000),
            pair_path: vec![1u8; 42],
            min_profit_onchain: U256::from(100),
            calldata: vec![1u8, 2, 3, 4],
            quoted_at_block: 100,
            expected_gross_profit: U256::from(300),
        }
    }

    #[test]
    fn produces_deterministic_hash_without_transaction_submission() {
        let p = prepared();
        let candidate = ReadOnlyCandidate::from_preflight(
            1, 100, &p, U256::from(150)
        ).unwrap();
        assert_eq!(candidate.calldata_hash, keccak256(&p.calldata));
        assert_eq!(candidate.pair_count, 2);
        assert_eq!(candidate.block, 100);
    }

    #[test]
    fn rejects_stale_wrong_chain_and_nonprofitable_candidates() {
        let p = prepared();
        assert!(ReadOnlyCandidate::from_preflight(8453, 100, &p, U256::from(1)).is_err());
        assert!(ReadOnlyCandidate::from_preflight(1, 101, &p, U256::from(1)).is_err());
        assert!(ReadOnlyCandidate::from_preflight(1, 100, &p, U256::ZERO).is_err());
        let mut no_data = p.clone();
        no_data.calldata.clear();
        assert!(ReadOnlyCandidate::from_preflight(1, 100, &no_data, U256::from(1)).is_err());
        let mut wrong_path = p;
        wrong_path.pair_path.pop();
        assert!(ReadOnlyCandidate::from_preflight(1, 100, &wrong_path, U256::from(1)).is_err());
    }
}
