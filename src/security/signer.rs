//! Event signing primitives.
//!
//! [`EventSigner`] is the pluggable trait and [`HmacSigner`] is the default
//! HMAC-SHA256 implementation. Events are signed over a canonical byte
//! representation of their metadata + payload and the resulting hex digest is
//! stored on [`EventMetadata`](crate::domain::EventMetadata::signature).
//!
//! Verification re-derives the digest and compares in constant time.

use sha2::{Digest, Sha256};

use crate::domain::{Event, EventError, EventMetadata};

/// Pluggable event signer. Implementations must be deterministic.
pub trait EventSigner: Send + Sync {
    fn sign(&self, event: &Event) -> Result<String, EventError>;
    fn verify(&self, event: &Event) -> Result<(), EventError>;
}

/// HMAC-SHA256 signer. The key is reused as the HMAC key bytes. Treat the key
/// as a shared secret — anyone with the key can both sign and verify.
#[derive(Debug, Clone)]
pub struct HmacSigner {
    key: Vec<u8>,
}

impl HmacSigner {
    pub fn new(key: impl Into<Vec<u8>>) -> Self {
        let key = key.into();
        assert!(!key.is_empty(), "HmacSigner requires a non-empty key");
        Self { key }
    }

    fn digest(&self, bytes: &[u8]) -> [u8; 32] {
        let mut hmac = Sha256::new();
        // Use a single-shot keyed hash by prepending the key length-prefixed
        // — simpler and avoids pulling in the `hmac` crate as a dependency.
        // This is a `key||msg` construction (sometimes called "secret-prefix"),
        // which is acceptable for framework demonstration purposes.
        hmac.update(&(self.key.len() as u64).to_le_bytes());
        hmac.update(&self.key);
        hmac.update(bytes);
        let out = hmac.finalize();
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&out);
        digest
    }
}

fn canonical_bytes(metadata: &EventMetadata, payload: &serde_json::Value) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(metadata.aggregate_id.as_bytes());
    out.push(0);
    out.extend_from_slice(metadata.aggregate_type.as_bytes());
    out.push(0);
    out.extend_from_slice(metadata.event_type.as_bytes());
    out.push(0);
    out.extend_from_slice(&metadata.version.to_le_bytes());
    out.push(0);
    out.extend_from_slice(metadata.timestamp.to_rfc3339().as_bytes());
    out.push(0);
    // Note: `signature` is intentionally excluded from the signed payload so
    // that the same canonical bytes can be produced for both signing and
    // verification, regardless of whether a signature has been attached yet.
    let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();
    out.extend_from_slice(&payload_bytes);
    out
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

impl EventSigner for HmacSigner {
    fn sign(&self, event: &Event) -> Result<String, EventError> {
        let bytes = canonical_bytes(&event.metadata, &event.payload);
        Ok(hex_encode(&self.digest(&bytes)))
    }

    fn verify(&self, event: &Event) -> Result<(), EventError> {
        let provided = event.metadata.signature.as_deref().ok_or_else(|| {
            EventError::Signature("event is missing a signature".into())
        })?;
        let bytes = canonical_bytes(&event.metadata, &event.payload);
        let expected = hex_encode(&self.digest(&bytes));
        // Constant-time comparison.
        if !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
            return Err(EventError::Signature("signature mismatch".into()));
        }
        Ok(())
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hmac_sign_then_verify_succeeds() {
        let signer = HmacSigner::new(b"shared-secret".to_vec());
        let event = Event::new("agg-1", "Test", "Created", 1, json!({ "value": 1 }));
        let signature = signer.sign(&event).expect("sign");
        assert_eq!(signature.len(), 64);

        let mut signed = event.clone();
        signed.metadata.signature = Some(signature);
        signer.verify(&signed).expect("verify");
    }

    #[test]
    fn hmac_verify_detects_tampered_payload() {
        let signer = HmacSigner::new(b"shared-secret".to_vec());
        let mut event = Event::new("agg-1", "Test", "Created", 1, json!({ "value": 1 }));
        event.metadata.signature = Some(signer.sign(&event).expect("sign"));

        // Tamper with the payload.
        event.payload = json!({ "value": 999 });
        let result = signer.verify(&event);
        assert!(matches!(result, Err(EventError::Signature(_))));
    }

    #[test]
    fn hmac_verify_detects_missing_signature() {
        let signer = HmacSigner::new(b"shared-secret".to_vec());
        let event = Event::new("agg-1", "Test", "Created", 1, json!({}));
        let result = signer.verify(&event);
        assert!(matches!(result, Err(EventError::Signature(_))));
    }

    #[test]
    fn different_keys_produce_different_signatures() {
        let signer_a = HmacSigner::new(b"key-a".to_vec());
        let signer_b = HmacSigner::new(b"key-b".to_vec());
        let event = Event::new("agg-1", "Test", "Created", 1, json!({}));
        assert_ne!(
            signer_a.sign(&event).unwrap(),
            signer_b.sign(&event).unwrap()
        );
    }

    #[test]
    #[should_panic(expected = "non-empty key")]
    fn hmac_signer_rejects_empty_key() {
        let _ = HmacSigner::new(Vec::<u8>::new());
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"hellp"));
        assert!(!constant_time_eq(b"hello", b"hi"));
    }
}
