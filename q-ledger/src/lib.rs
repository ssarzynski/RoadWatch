use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub type Hash = [u8; 32];
pub const GENESIS_PREVIOUS_HASH: Hash = [0_u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LedgerEvent {
    pub sequence: u64,
    pub event_type: String,
    /// SHA-256 commitment to the canonical private/source record.
    pub subject_commitment: String,
    pub previous_event_hash: String,
    /// Server-controlled Unix timestamp; not a client-provided time.
    pub recorded_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedCheckpoint {
    pub tree_size: u64,
    pub merkle_root: String,
    pub last_event_hash: String,
    pub created_at_unix: i64,
    pub public_key: String,
    pub signature: String,
}

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("serialization failed")]
    Serialization,
    #[error("invalid hex")]
    InvalidHex,
    #[error("invalid public key")]
    InvalidPublicKey,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("checkpoint signature verification failed")]
    VerificationFailed,
}

pub fn sha256(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}

pub fn hex_hash(hash: Hash) -> String {
    hex::encode(hash)
}

pub fn event_hash(event: &LedgerEvent) -> Result<Hash, LedgerError> {
    // Struct serialization fixes field order. Versioning this event schema is
    // required before changing fields in production.
    let canonical = serde_json::to_vec(event).map_err(|_| LedgerError::Serialization)?;
    Ok(sha256(&canonical))
}

pub fn verify_chain(events: &[LedgerEvent]) -> Result<bool, LedgerError> {
    let mut expected_previous = GENESIS_PREVIOUS_HASH;
    for (index, event) in events.iter().enumerate() {
        if event.sequence != index as u64 {
            return Ok(false);
        }
        if event.previous_event_hash != hex_hash(expected_previous) {
            return Ok(false);
        }
        expected_previous = event_hash(event)?;
    }
    Ok(true)
}

pub fn merkle_root(leaves: &[Hash]) -> Hash {
    if leaves.is_empty() {
        return sha256(b"roadwatch-q-ledger-empty-v1");
    }

    let mut level = leaves.to_vec();
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            let last = *level.last().expect("non-empty");
            level.push(last);
        }
        level = level
            .chunks_exact(2)
            .map(|pair| {
                let mut data = Vec::with_capacity(65);
                data.push(0x01); // domain separation for internal nodes
                data.extend_from_slice(&pair[0]);
                data.extend_from_slice(&pair[1]);
                sha256(&data)
            })
            .collect();
    }
    level[0]
}

fn checkpoint_message(tree_size: u64, root: &str, last: &str, created: i64) -> Vec<u8> {
    format!("roadwatch-q-ledger-checkpoint-v1\n{tree_size}\n{root}\n{last}\n{created}\n").into_bytes()
}

pub fn sign_checkpoint(
    events: &[LedgerEvent],
    signing_key: &SigningKey,
    created_at_unix: i64,
) -> Result<SignedCheckpoint, LedgerError> {
    let hashes: Result<Vec<_>, _> = events.iter().map(event_hash).collect();
    let hashes = hashes?;
    let root = hex_hash(merkle_root(&hashes));
    let last = hashes.last().copied().unwrap_or(GENESIS_PREVIOUS_HASH);
    let last = hex_hash(last);
    let msg = checkpoint_message(events.len() as u64, &root, &last, created_at_unix);
    let signature = signing_key.sign(&msg);

    Ok(SignedCheckpoint {
        tree_size: events.len() as u64,
        merkle_root: root,
        last_event_hash: last,
        created_at_unix,
        public_key: hex::encode(signing_key.verifying_key().to_bytes()),
        signature: hex::encode(signature.to_bytes()),
    })
}

pub fn verify_checkpoint(checkpoint: &SignedCheckpoint) -> Result<(), LedgerError> {
    let pk: [u8; 32] = hex::decode(&checkpoint.public_key)
        .map_err(|_| LedgerError::InvalidHex)?
        .try_into()
        .map_err(|_| LedgerError::InvalidPublicKey)?;
    let sig: [u8; 64] = hex::decode(&checkpoint.signature)
        .map_err(|_| LedgerError::InvalidHex)?
        .try_into()
        .map_err(|_| LedgerError::InvalidSignature)?;

    let key = VerifyingKey::from_bytes(&pk).map_err(|_| LedgerError::InvalidPublicKey)?;
    let signature = Signature::from_bytes(&sig);
    let msg = checkpoint_message(
        checkpoint.tree_size,
        &checkpoint.merkle_root,
        &checkpoint.last_event_hash,
        checkpoint.created_at_unix,
    );
    key.verify(&msg, &signature)
        .map_err(|_| LedgerError::VerificationFailed)
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WitnessStatement {
    pub checkpoint_tree_size: u64,
    pub checkpoint_merkle_root: String,
    pub checkpoint_signature: String,
    pub witness_public_key: String,
    pub witnessed_at_unix: i64,
    pub witness_signature: String,
}

fn witness_message(checkpoint: &SignedCheckpoint, witnessed_at_unix: i64) -> Vec<u8> {
    format!(
        "roadwatch-q-ledger-witness-v1\n{}\n{}\n{}\n{}\n",
        checkpoint.tree_size,
        checkpoint.merkle_root,
        checkpoint.signature,
        witnessed_at_unix
    )
    .into_bytes()
}

pub fn verify_checkpoint_against_events(
    events: &[LedgerEvent],
    checkpoint: &SignedCheckpoint,
) -> Result<(), LedgerError> {
    if !verify_chain(events)? || checkpoint.tree_size != events.len() as u64 {
        return Err(LedgerError::VerificationFailed);
    }
    let hashes: Result<Vec<_>, _> = events.iter().map(event_hash).collect();
    let hashes = hashes?;
    let expected_root = hex_hash(merkle_root(&hashes));
    let expected_last = hex_hash(hashes.last().copied().unwrap_or(GENESIS_PREVIOUS_HASH));
    if checkpoint.merkle_root != expected_root || checkpoint.last_event_hash != expected_last {
        return Err(LedgerError::VerificationFailed);
    }
    verify_checkpoint(checkpoint)
}

pub fn witness_checkpoint(
    events: &[LedgerEvent],
    checkpoint: &SignedCheckpoint,
    witness_key: &SigningKey,
    witnessed_at_unix: i64,
) -> Result<WitnessStatement, LedgerError> {
    verify_checkpoint_against_events(events, checkpoint)?;
    let msg = witness_message(checkpoint, witnessed_at_unix);
    let signature = witness_key.sign(&msg);
    Ok(WitnessStatement {
        checkpoint_tree_size: checkpoint.tree_size,
        checkpoint_merkle_root: checkpoint.merkle_root.clone(),
        checkpoint_signature: checkpoint.signature.clone(),
        witness_public_key: hex::encode(witness_key.verifying_key().to_bytes()),
        witnessed_at_unix,
        witness_signature: hex::encode(signature.to_bytes()),
    })
}

pub fn verify_witness(
    checkpoint: &SignedCheckpoint,
    statement: &WitnessStatement,
) -> Result<(), LedgerError> {
    if statement.checkpoint_tree_size != checkpoint.tree_size
        || statement.checkpoint_merkle_root != checkpoint.merkle_root
        || statement.checkpoint_signature != checkpoint.signature
    {
        return Err(LedgerError::VerificationFailed);
    }
    let pk: [u8; 32] = hex::decode(&statement.witness_public_key)
        .map_err(|_| LedgerError::InvalidHex)?
        .try_into()
        .map_err(|_| LedgerError::InvalidPublicKey)?;
    let sig: [u8; 64] = hex::decode(&statement.witness_signature)
        .map_err(|_| LedgerError::InvalidHex)?
        .try_into()
        .map_err(|_| LedgerError::InvalidSignature)?;
    let key = VerifyingKey::from_bytes(&pk).map_err(|_| LedgerError::InvalidPublicKey)?;
    let signature = Signature::from_bytes(&sig);
    key.verify(
        &witness_message(checkpoint, statement.witnessed_at_unix),
        &signature,
    )
    .map_err(|_| LedgerError::VerificationFailed)
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointComparison {
    Identical,
    SameLogGrowth,
    Equivocation,
    DifferentSigner,
}

pub fn compare_checkpoints(
    trusted: &SignedCheckpoint,
    observed: &SignedCheckpoint,
) -> Result<CheckpointComparison, LedgerError> {
    verify_checkpoint(trusted)?;
    verify_checkpoint(observed)?;

    if trusted.public_key != observed.public_key {
        return Ok(CheckpointComparison::DifferentSigner);
    }

    if trusted.tree_size == observed.tree_size {
        if trusted.merkle_root == observed.merkle_root
            && trusted.last_event_hash == observed.last_event_hash
        {
            return Ok(CheckpointComparison::Identical);
        }
        return Ok(CheckpointComparison::Equivocation);
    }

    // Different sizes are not themselves proof of consistency. A future
    // consistency proof must establish append-only growth between roots.
    Ok(CheckpointComparison::SameLogGrowth)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(sequence: u64, previous: Hash, commitment: &str) -> LedgerEvent {
        LedgerEvent {
            sequence,
            event_type: "camera_status_changed".into(),
            subject_commitment: commitment.into(),
            previous_event_hash: hex_hash(previous),
            recorded_at_unix: 1_700_000_000 + sequence as i64,
        }
    }

    #[test]
    fn detects_rewritten_chain() {
        let first = event(0, GENESIS_PREVIOUS_HASH, "a");
        let first_hash = event_hash(&first).unwrap();
        let mut second = event(1, first_hash, "b");
        assert!(verify_chain(&[first.clone(), second.clone()]).unwrap());
        second.subject_commitment = "rewritten".into();
        let third = event(2, event_hash(&second).unwrap(), "c");
        assert!(!verify_chain(&[first, second, third]).unwrap());
    }

    #[test]
    fn merkle_root_is_deterministic() {
        let leaves = vec![sha256(b"a"), sha256(b"b"), sha256(b"c")];
        assert_eq!(merkle_root(&leaves), merkle_root(&leaves));
    }

    #[test]
    fn signed_checkpoint_verifies_and_tamper_fails() {
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let first = event(0, GENESIS_PREVIOUS_HASH, "a");
        let mut checkpoint = sign_checkpoint(&[first], &key, 1_700_000_100).unwrap();
        verify_checkpoint(&checkpoint).unwrap();
        checkpoint.tree_size += 1;
        assert!(verify_checkpoint(&checkpoint).is_err());
    }
    #[test]
    fn independent_witness_verifies_checkpoint_and_detects_substitution() {
        let log_key = SigningKey::from_bytes(&[7_u8; 32]);
        let witness_key = SigningKey::from_bytes(&[9_u8; 32]);
        let first = event(0, GENESIS_PREVIOUS_HASH, "a");
        let events = vec![first];
        let checkpoint = sign_checkpoint(&events, &log_key, 1_700_000_100).unwrap();
        let statement =
            witness_checkpoint(&events, &checkpoint, &witness_key, 1_700_000_120).unwrap();
        verify_witness(&checkpoint, &statement).unwrap();

        let mut substituted = checkpoint.clone();
        substituted.merkle_root = hex_hash(sha256(b"substitution"));
        assert!(verify_witness(&substituted, &statement).is_err());
    }


    #[test]
    fn same_signer_same_size_different_valid_roots_is_equivocation() {
        let key = SigningKey::from_bytes(&[7_u8; 32]);
        let a = event(0, GENESIS_PREVIOUS_HASH, "history-a");
        let b = event(0, GENESIS_PREVIOUS_HASH, "history-b");
        let checkpoint_a = sign_checkpoint(&[a], &key, 1_700_000_100).unwrap();
        let checkpoint_b = sign_checkpoint(&[b], &key, 1_700_000_101).unwrap();
        assert_eq!(
            compare_checkpoints(&checkpoint_a, &checkpoint_b).unwrap(),
            CheckpointComparison::Equivocation
        );
    }

    #[test]
    fn different_signers_are_not_called_equivocation() {
        let key_a = SigningKey::from_bytes(&[7_u8; 32]);
        let key_b = SigningKey::from_bytes(&[8_u8; 32]);
        let event = event(0, GENESIS_PREVIOUS_HASH, "same");
        let a = sign_checkpoint(&[event.clone()], &key_a, 1_700_000_100).unwrap();
        let b = sign_checkpoint(&[event], &key_b, 1_700_000_100).unwrap();
        assert_eq!(
            compare_checkpoints(&a, &b).unwrap(),
            CheckpointComparison::DifferentSigner
        );
    }


}
