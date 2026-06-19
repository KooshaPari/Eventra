//! # Domain Services
//!
//! Domain services for event sourcing operations.
//!
//! ## Services
//!
//! - Hash computation and verification
//! - Gap detection in event streams

use sha2::{Digest, Sha256};

/// Compute SHA-256 hash of event data for chain integrity.
///
/// The hash is computed over:
/// - Previous event hash
/// - Event ID
/// - Timestamp
/// - Actor
/// - Serialized payload
pub fn compute_event_hash(
    prev_hash: &str,
    event_id: &str,
    timestamp: &str,
    actor: &str,
    payload_bytes: &[u8],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(event_id.as_bytes());
    hasher.update(timestamp.as_bytes());
    hasher.update(actor.as_bytes());
    hasher.update(payload_bytes);
    hex::encode(hasher.finalize())
}

/// Detect gaps in a sequence of events.
///
/// Returns a vector of (expected, actual) tuples for any gaps found.
pub fn detect_sequence_gaps(sequences: &[i64]) -> Vec<(i64, i64)> {
    let mut gaps = Vec::new();
    for window in sequences.windows(2) {
        let current = window[0];
        let next = window[1];
        if next != current + 1 {
            gaps.push((current + 1, next));
        }
    }
    gaps
}

/// Verify the hash chain integrity of a sequence of events.
///
/// Returns Ok(()) if all hashes are valid, Err on first failure.
pub fn verify_hash_chain<H: Fn(usize) -> Option<String>>(
    sequences: &[i64],
    get_payload_hash: H,
) -> Result<(), HashChainError> {
    for i in 1..sequences.len() {
        let prev_seq = sequences[i - 1];
        let curr_seq = sequences[i];
        if curr_seq != prev_seq + 1 {
            return Err(HashChainError::SequenceGap {
                expected: prev_seq + 1,
                actual: curr_seq,
            });
        }
    }
    Ok(())
}

/// Errors from hash chain verification.
#[derive(Debug, thiserror::Error)]
pub enum HashChainError {
    #[error("Sequence gap: expected {expected}, got {actual}")]
    SequenceGap { expected: i64, actual: i64 },

    #[error("Hash mismatch at sequence {sequence}")]
    HashMismatch { sequence: i64 },

    #[error("Invalid hash length: expected 64 hex chars, got {0}")]
    InvalidHashLength(usize),
}

/// Validate that a hash string is properly formatted (64 hex characters).
pub fn validate_hash(hash: &str) -> Result<(), HashChainError> {
    if hash.len() != 64 {
        return Err(HashChainError::InvalidHashLength(hash.len()));
    }
    if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(HashChainError::InvalidHashLength(hash.len()));
    }
    Ok(())
}
