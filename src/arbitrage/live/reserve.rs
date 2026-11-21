//! Durable single-use reservation for the $5 one-shot gate.
//!
//! The gate demands proof that the EXACT candidate (calldata hash, sender,
//! nonce, fee cap) is the one and only persistent attempt. A boolean is not
//! evidence, so the proof lives on disk and is re-read before submission.
//!
//! Durability: the file is created with `create_new` (atomic O_EXCL), flushed
//! with `sync_all` and the parent directory is synced. A crash either leaves
//! the reservation or does not; it can never leave a half-written duplicate.
//!
//! Scope: single-host exclusive. Two workers on different hosts need a shared
//! filesystem or Postgres.
use alloy::primitives::{keccak256, Address, B256, U256};
use anyhow::{bail, ensure, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const EXT: &str = "res";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReservationKey {
    pub chain_id: u64,
    pub sender: Address,
    pub nonce: u64,
    pub calldata_hash: B256,
    pub max_fee_per_gas_wei: U256,
    pub gas_limit: u64,
}

impl ReservationKey {
    /// Deterministic, order-stable serialisation. Any field change alters both
    /// the identity hash and the persisted bytes.
    fn canonical(&self) -> String {
        format!(
            "chain_id={}\nsender={}\nnonce={}\ncalldata_hash={}\n\
             max_fee_per_gas_wei={:#x}\ngas_limit={}\n",
            self.chain_id,
            self.sender,
            self.nonce,
            self.calldata_hash,
            self.max_fee_per_gas_wei,
            self.gas_limit
        )
    }

    pub fn id(&self) -> B256 {
        keccak256(self.canonical().as_bytes())
    }
}

#[derive(Debug)]
pub struct Reservation {
    pub id: B256,
    pub path: PathBuf,
    key: ReservationKey,
}

/// Claim the single use of `key`. Fails if this exact candidate was ever
/// reserved before, even across process restarts.
pub fn acquire(dir: &Path, key: &ReservationKey) -> Result<Reservation> {
    ensure!(key.chain_id == 1, "reservation is Ethereum mainnet only");
    ensure!(key.sender != Address::ZERO, "reservation needs a sender");
    ensure!(key.nonce > 0 || key.gas_limit > 0, "reservation needs a candidate");
    fs::create_dir_all(dir)
        .with_context(|| format!("cannot create reservation dir {}", dir.display()))?;
    let id = key.id();
    let path = dir.join(format!("{id:#x}.{EXT}"));
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!("candidate already reserved: {}", path.display())
        }
        Err(error) => {
            return Err(error).with_context(|| format!("cannot create {}", path.display()))
        }
    };
    file.write_all(key.canonical().as_bytes())
        .with_context(|| format!("cannot write {}", path.display()))?;
    file.sync_all()?;
    drop(file);
    sync_dir(dir);
    Ok(Reservation { id, path, key: key.clone() })
}

impl Reservation {
    /// Re-read the persisted bytes and compare. This is the
    /// `reservation_committed_and_exact` evidence handed to the gate.
    pub fn is_exact(&self) -> bool {
        fs::read_to_string(&self.path)
            .map(|stored| stored == self.key.canonical())
            .unwrap_or(false)
    }
}

fn sync_dir(dir: &Path) {
    if let Ok(handle) = File::open(dir) {
        let _ = handle.sync_all();
    }
}

/// Every reservation ever made in `dir`. The gate accepts only `1`, so archive
/// the directory to authorise a fresh single shot.
pub fn count_attempts(dir: &Path) -> Result<u64> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let mut total = 0u64;
    for entry in entries {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == EXT) {
            total += 1;
        }
    }
    Ok(total)
}

/// Move every reservation into `into` and return how many were archived.
pub fn archive_all(dir: &Path, into: &Path) -> Result<u64> {
    if !dir.exists() {
        return Ok(0);
    }
    fs::create_dir_all(into)?;
    let mut moved = 0u64;
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == EXT) {
            fs::rename(&path, into.join(path.file_name().unwrap()))?;
            moved += 1;
        }
    }
    sync_dir(dir);
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, keccak256};

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "sandooo-res-{}-{tag}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn key(nonce: u64) -> ReservationKey {
        ReservationKey {
            chain_id: 1,
            sender: address!("00000000000000000000000000000000000000aa"),
            nonce,
            calldata_hash: keccak256([nonce as u8]),
            max_fee_per_gas_wei: U256::from(10_000_000_000u64),
            gas_limit: 400_000,
        }
    }

    #[test]
    fn first_acquire_persists_exact_evidence() {
        let dir = temp_dir("first");
        let reservation = acquire(&dir, &key(7)).unwrap();
        assert!(reservation.is_exact());
        assert_eq!(count_attempts(&dir).unwrap(), 1);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn same_candidate_cannot_be_reserved_twice() {
        let dir = temp_dir("dup");
        acquire(&dir, &key(7)).unwrap();
        let error = acquire(&dir, &key(7)).unwrap_err().to_string();
        assert!(error.contains("already reserved"), "{error}");
        assert_eq!(count_attempts(&dir).unwrap(), 1);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn identity_changes_with_every_field() {
        let base = key(7);
        let mut other_nonce = base.clone();
        other_nonce.nonce = 8;
        let mut other_fee = base.clone();
        other_fee.max_fee_per_gas_wei = U256::from(11_000_000_000u64);
        let mut other_sender = base.clone();
        other_sender.sender = address!("00000000000000000000000000000000000000bb");
        let mut other_data = base.clone();
        other_data.calldata_hash = keccak256([9u8]);
        let mut other_gas = base.clone();
        other_gas.gas_limit = 1;
        for other in [other_nonce, other_fee, other_sender, other_data, other_gas] {
            assert_ne!(base.id(), other.id());
        }
        assert_eq!(base.id(), key(7).id());
    }

    #[test]
    fn tampered_file_is_not_exact() {
        let dir = temp_dir("tamper");
        let reservation = acquire(&dir, &key(7)).unwrap();
        fs::write(&reservation.path, "chain_id=1\n").unwrap();
        assert!(!reservation.is_exact());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn archiving_resets_the_single_shot() {
        let dir = temp_dir("archive");
        let into = dir.join("old");
        acquire(&dir, &key(7)).unwrap();
        acquire(&dir, &key(8)).unwrap();
        assert_eq!(count_attempts(&dir).unwrap(), 2);
        assert_eq!(archive_all(&dir, &into).unwrap(), 2);
        assert_eq!(count_attempts(&dir).unwrap(), 0);
        acquire(&dir, &key(9)).unwrap();
        assert_eq!(count_attempts(&dir).unwrap(), 1);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn missing_directory_counts_as_zero() {
        let dir = temp_dir("missing").join("nope");
        assert_eq!(count_attempts(&dir).unwrap(), 0);
    }
}
