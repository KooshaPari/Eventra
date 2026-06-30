//! SHA-256 hash chain computation and verification.

use chrono::{DateTime, Utc};
use hex::FromHex;
use sha2::{Digest, Sha256};

use crate::error::HashError;

/// Compute SHA-256 hash for an event.
///
/// Hash inputs (in order, length-prefixed where noted):
/// 1. UUID (16 bytes)
/// 2. timestamp (length-prefixed ISO 8601)
/// 3. event_type (length-prefixed UTF-8)
/// 4. payload (length-prefixed JSON)
/// 5. actor (length-prefixed UTF-8)
/// 6. prev_hash (64 hex chars = 32 bytes decoded)
pub fn compute_hash(
    id: &uuid::Uuid,
    timestamp: DateTime<Utc>,
    event_type: &str,
    payload: &serde_json::Value,
    actor: &str,
    prev_hash: &str,
) -> Result<String, HashError> {
    let mut hasher = Sha256::new();

    // UUID bytes (16 bytes)
    hasher.update(id.as_bytes());

    // Timestamp (ISO 8601 string)
    let timestamp_str = timestamp.to_rfc3339();
    hasher.update((timestamp_str.len() as u32).to_be_bytes());
    hasher.update(timestamp_str.as_bytes());

    // Event type
    hasher.update((event_type.len() as u32).to_be_bytes());
    hasher.update(event_type.as_bytes());

    // Payload (JSON)
    let payload_json =
        serde_json::to_string(payload).map_err(|_| HashError::InvalidHashLength(0))?;
    hasher.update((payload_json.len() as u32).to_be_bytes());
    hasher.update(payload_json.as_bytes());

    // Actor
    hasher.update((actor.len() as u32).to_be_bytes());
    hasher.update(actor.as_bytes());

    // Previous hash (decode from hex)
    let prev_bytes = <Vec<u8>>::from_hex(prev_hash)
        .map_err(|_| HashError::InvalidHashLength(prev_hash.len()))?;
    if prev_bytes.len() != 32 {
        return Err(HashError::InvalidHashLength(prev_bytes.len()));
    }
    hasher.update(&prev_bytes);

    let result = hasher.finalize();
    Ok(hex::encode(result))
}

/// Verify the integrity of an event chain.
///
/// Ensures each event's hash is correctly computed and chains to its predecessor.
pub fn verify_chain(events: &[(String, String)]) -> Result<(), HashError> {
    if events.is_empty() {
        return Ok(());
    }

    // First event must chain from zero hash
    let zero_hash = "0".repeat(64);
    if events[0].1 != zero_hash {
        return Err(HashError::ChainBroken { sequence: 1 });
    }

    // Verify sequence continuity and hashes
    for (i, (_hash, prev_hash)) in events.iter().enumerate() {
        if i == 0 {
            continue;
        }
        let seq = (i + 1) as i64;
        if prev_hash != &events[i - 1].0 {
            return Err(HashError::ChainBroken { sequence: seq });
        }
    }

    Ok(())
}

/// Detect gaps in a sequence of events.
///
/// Returns the first missing sequence number, or None if the sequence is continuous.
pub fn detect_gaps(sequences: &[i64]) -> Option<i64> {
    if sequences.is_empty() {
        return None;
    }

    let mut sorted = sequences.to_vec();
    sorted.sort_unstable();

    for i in 1..sorted.len() {
        if sorted[i] != sorted[i - 1] + 1 {
            return Some(sorted[i - 1] + 1);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_hash_deterministic() {
        let id = uuid::Uuid::nil();
        let ts = DateTime::parse_from_rfc3339("2026-03-02T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let payload = serde_json::json!({"n": "t"});
        let zero_hash = "0".repeat(64);

        let h1 = compute_hash(&id, ts, "created", &payload, "u1", &zero_hash).unwrap();
        let h2 = compute_hash(&id, ts, "created", &payload, "u1", &zero_hash).unwrap();

        assert_eq!(h1, h2);
        assert_ne!(h1, zero_hash);
    }

    #[test]
    fn compute_hash_changes_with_payload() {
        let id = uuid::Uuid::nil();
        let ts = DateTime::parse_from_rfc3339("2026-03-02T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let zero_hash = "0".repeat(64);

        let h1 = compute_hash(
            &id,
            ts,
            "created",
            &serde_json::json!({"n": "t"}),
            "u1",
            &zero_hash,
        )
        .unwrap();
        let h2 = compute_hash(
            &id,
            ts,
            "created",
            &serde_json::json!({"n": "x"}),
            "u1",
            &zero_hash,
        )
        .unwrap();

        assert_ne!(h1, h2);
    }

    #[test]
    fn verify_chain_empty() {
        verify_chain(&[]).unwrap();
    }

    #[test]
    fn verify_chain_single() {
        let zero_hash = "0".repeat(64);
        let hash = "abc123".to_string();
        verify_chain(&[(hash, zero_hash)]).unwrap();
    }

    #[test]
    fn verify_chain_two_events() {
        let zero_hash = "0".repeat(64);
        let h1 = "abc123".to_string();
        let h2 = "def456".to_string();

        verify_chain(&[(h1.clone(), zero_hash), (h2, h1)]).unwrap();
    }

    #[test]
    fn detect_gaps_no_gap() {
        assert_eq!(detect_gaps(&[1, 2, 3, 4, 5]), None);
    }

    #[test]
    fn detect_gaps_with_gap() {
        assert_eq!(detect_gaps(&[1, 2, 4, 5]), Some(3));
    }

    #[test]
    fn detect_gaps_empty() {
        assert_eq!(detect_gaps(&[]), None);
    }

    // --- Property / invariant tests (table-driven) ---
    //
    // These tests encode the semantic invariants of the hash chain without
    // requiring an external property-testing crate. Each table row is an
    // independent scenario designed to exercise a distinct corner-case.

    /// Invariant: `compute_hash` output is exactly 64 hex characters (32-byte SHA-256).
    #[test]
    fn hash_output_is_always_64_hex_chars() {
        let zero_hash = "0".repeat(64);
        let id = uuid::Uuid::nil();
        let ts = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let cases: &[(&str, &str, &str)] = &[
            ("created", "{}", "user-a"),
            ("updated", r#"{"k":"v"}"#, "user-b"),
            ("deleted", "null", ""),
            // Empty strings for event_type and actor are edge inputs
            ("", "{}", ""),
        ];

        for (event_type, payload_str, actor) in cases {
            let payload: serde_json::Value = serde_json::from_str(payload_str).unwrap();
            let h = compute_hash(&id, ts, event_type, &payload, actor, &zero_hash).unwrap();
            assert_eq!(
                h.len(),
                64,
                "hash must be 64 hex chars for event_type={event_type:?}"
            );
            assert!(
                h.chars().all(|c| c.is_ascii_hexdigit()),
                "hash must be lowercase hex for event_type={event_type:?}"
            );
        }
    }

    /// Invariant: changing any single input field produces a different hash
    /// (collision-resistance for each dimension individually).
    #[test]
    fn hash_changes_on_each_input_dimension() {
        let base_id = uuid::Uuid::nil();
        let base_ts = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let base_payload = serde_json::json!({"x": 1});
        let zero_hash = "0".repeat(64);

        let base =
            compute_hash(&base_id, base_ts, "evt", &base_payload, "actor", &zero_hash).unwrap();

        // Changed UUID
        let other_id = uuid::Uuid::from_u128(1);
        let h = compute_hash(
            &other_id,
            base_ts,
            "evt",
            &base_payload,
            "actor",
            &zero_hash,
        )
        .unwrap();
        assert_ne!(base, h, "hash must differ when UUID changes");

        // Changed timestamp
        let other_ts = DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let h = compute_hash(
            &base_id,
            other_ts,
            "evt",
            &base_payload,
            "actor",
            &zero_hash,
        )
        .unwrap();
        assert_ne!(base, h, "hash must differ when timestamp changes");

        // Changed event_type
        let h = compute_hash(
            &base_id,
            base_ts,
            "other",
            &base_payload,
            "actor",
            &zero_hash,
        )
        .unwrap();
        assert_ne!(base, h, "hash must differ when event_type changes");

        // Changed payload
        let other_payload = serde_json::json!({"x": 2});
        let h = compute_hash(
            &base_id,
            base_ts,
            "evt",
            &other_payload,
            "actor",
            &zero_hash,
        )
        .unwrap();
        assert_ne!(base, h, "hash must differ when payload changes");

        // Changed actor
        let h = compute_hash(&base_id, base_ts, "evt", &base_payload, "other", &zero_hash).unwrap();
        assert_ne!(base, h, "hash must differ when actor changes");

        // Changed prev_hash (simulate different predecessor)
        let alt_prev = "a".repeat(64);
        let h = compute_hash(&base_id, base_ts, "evt", &base_payload, "actor", &alt_prev).unwrap();
        assert_ne!(base, h, "hash must differ when prev_hash changes");
    }

    /// Invariant: `verify_chain` detects a tampered link anywhere in a chain
    /// of N events.
    #[test]
    fn verify_chain_detects_any_tampered_link() {
        // Build a 5-event chain where `prev_hash` of event[i] is event[i-1]'s hash.
        let zero_hash = "0".repeat(64);
        let hashes = vec![
            "aaaa".repeat(16),
            "bbbb".repeat(16),
            "cccc".repeat(16),
            "dddd".repeat(16),
            "eeee".repeat(16),
        ];

        // Correct chain: (hash, prev_hash)
        let chain: Vec<(String, String)> = std::iter::once((hashes[0].clone(), zero_hash.clone()))
            .chain(hashes.windows(2).map(|w| (w[1].clone(), w[0].clone())))
            .collect();

        assert!(
            verify_chain(&chain).is_ok(),
            "well-formed chain must verify cleanly"
        );

        // Tamper each link in turn and expect an error.
        for tamper_idx in 1..chain.len() {
            let mut tampered = chain.clone();
            // Break the back-pointer of the entry at tamper_idx
            tampered[tamper_idx].1 = "ffff".repeat(16);
            assert!(
                verify_chain(&tampered).is_err(),
                "tampering link at index {tamper_idx} must be detected"
            );
        }
    }

    /// Invariant: `detect_gaps` finds the *first* missing sequence number in
    /// out-of-order inputs.
    #[test]
    fn detect_gaps_table_driven() {
        let cases: &[(&[i64], Option<i64>)] = &[
            (&[1], None),
            (&[1, 2, 3], None),
            (&[3, 1, 2], None),       // unsorted input
            (&[1, 3], Some(2)),       // gap at 2
            (&[1, 2, 4, 5], Some(3)), // gap at 3
            (&[2, 4], Some(3)),       // gap between non-1 start
        ];

        for (seqs, expected) in cases {
            assert_eq!(
                detect_gaps(seqs),
                *expected,
                "detect_gaps({seqs:?}) != {expected:?}"
            );
        }
    }
}
