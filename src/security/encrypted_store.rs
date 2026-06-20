//! [`EncryptedEventStore`] — [`EventStore`](crate::domain::EventStore) adapter
//! that encrypts event payloads at rest.
//!
//! The wrapper stores each event's payload as ciphertext (a JSON string
//! containing base64) while keeping metadata in cleartext so that
//! [`EventStore::get_events_since`](crate::domain::EventStore::get_events_since)
//! and projection replay still work without decrypting every event.

use std::sync::Arc;

use crate::domain::{Event, EventError, EventStore};

use super::cipher::Cipher;

/// Encrypts/decrypts the `payload` field of each [`Event`] using a pluggable
/// [`Cipher`]. Metadata (ids, version, timestamp, type) is stored in cleartext.
pub struct EncryptedEventStore {
    inner: Arc<dyn EventStore>,
    cipher: Arc<dyn Cipher>,
}

impl EncryptedEventStore {
    pub fn new(inner: Arc<dyn EventStore>, cipher: Arc<dyn Cipher>) -> Self {
        Self { inner, cipher }
    }

    fn encrypt_event(&self, event: &Event) -> Result<Event, EventError> {
        let plaintext = serde_json::to_vec(&event.payload)?;
        let ciphertext = self.cipher.encrypt(&plaintext)?;
        let encoded = serde_json::Value::String(base64_encode(&ciphertext));
        Ok(Event {
            metadata: event.metadata.clone(),
            payload: serde_json::json!({ "enc": encoded }),
        })
    }

    fn decrypt_event(&self, event: &Event) -> Result<Event, EventError> {
        let encoded = event
            .payload
            .get("enc")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                EventError::Encryption("event payload is not an encrypted envelope".into())
            })?;
        let ciphertext = base64_decode(encoded)?;
        let plaintext = self.cipher.decrypt(&ciphertext)?;
        let payload: serde_json::Value = serde_json::from_slice(&plaintext)?;
        Ok(Event {
            metadata: event.metadata.clone(),
            payload,
        })
    }
}

impl EventStore for EncryptedEventStore {
    fn append(&self, event: &Event) -> Result<(), EventError> {
        let encrypted = self.encrypt_event(event)?;
        self.inner.append(&encrypted)
    }

    fn get_events(&self, aggregate_id: &str) -> Result<Vec<Event>, EventError> {
        self.inner
            .get_events(aggregate_id)?
            .iter()
            .map(|e| self.decrypt_event(e))
            .collect()
    }

    fn get_events_since(
        &self,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Event>, EventError> {
        // Timestamps are metadata, so we can filter without decrypting.
        self.inner
            .get_events_since(since)?
            .iter()
            .map(|e| self.decrypt_event(e))
            .collect()
    }

    fn get_all_events(&self) -> Result<Vec<Event>, EventError> {
        self.inner
            .get_all_events()?
            .iter()
            .map(|e| self.decrypt_event(e))
            .collect()
    }
}

/// Minimal RFC 4648 base64 (standard alphabet, with padding).
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    let mut chunks = input.chunks(3);
    while let Some(chunk) = chunks.next() {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(ALPHABET[(b0 >> 2) as usize] as char);
        out.push(ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode(input: &str) -> Result<Vec<u8>, EventError> {
    const ALPHABET: &[u8; 128] = &{
        let mut t = [255u8; 128];
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0;
        while i < alphabet.len() {
            t[alphabet[i] as usize] = i as u8;
            i += 1;
        }
        t
    };
    let bytes = input.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err(EventError::Encryption("invalid base64 length".into()));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let mut i = 0;
    while i < bytes.len() {
        let c0 = decode_char(ALPHABET, bytes[i])?;
        let c1 = decode_char(ALPHABET, bytes[i + 1])?;
        out.push((c0 << 2) | (c1 >> 4));
        if bytes[i + 2] != b'=' {
            let c2 = decode_char(ALPHABET, bytes[i + 2])?;
            out.push(((c1 & 0x0f) << 4) | (c2 >> 2));
            if bytes[i + 3] != b'=' {
                let c3 = decode_char(ALPHABET, bytes[i + 3])?;
                out.push(((c2 & 0x03) << 6) | c3);
            }
        }
        i += 4;
    }
    Ok(out)
}

fn decode_char(table: &[u8; 128], c: u8) -> Result<u8, EventError> {
    if c >= 128 {
        return Err(EventError::Encryption("invalid base64 character".into()));
    }
    let value = table[c as usize];
    if value == 255 {
        Err(EventError::Encryption("invalid base64 character".into()))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use crate::adapters::event_store::InMemoryEventStore;
    use crate::domain::Event;

    use super::*;

    #[test]
    fn base64_roundtrips_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_decode_roundtrips() {
        let cases: &[(&[u8], &str)] = &[
            (b"", ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foobar", "Zm9vYmFy"),
        ];
        for (expected, encoded) in cases {
            assert_eq!(&base64_decode(encoded).expect("decode"), expected);
        }
    }

    #[test]
    fn encrypted_store_roundtrips_payloads() {
        let inner = Arc::new(InMemoryEventStore::new());
        let cipher = Arc::new(super::super::XorCipher::new(b"k".to_vec()));
        let store = EncryptedEventStore::new(inner.clone(), cipher);

        let event = Event::new("agg-1", "Test", "Created", 1, json!({ "secret": 42 }));
        store.append(&event).expect("append");

        let plaintext = inner
            .get_events("agg-1")
            .expect("read raw")
            .into_iter()
            .next()
            .expect("one event");
        // The inner store should not contain the raw payload.
        assert!(plaintext.payload.get("secret").is_none());
        assert!(plaintext.payload.get("enc").is_some());

        // The wrapper decrypts transparently.
        let decrypted = store.get_events("agg-1").expect("decrypt").remove(0);
        assert_eq!(decrypted.payload, json!({ "secret": 42 }));
        assert_eq!(decrypted.metadata.event_type, "Created");
    }

    #[test]
    fn encrypted_store_rejects_non_envelope_payloads() {
        let inner = Arc::new(InMemoryEventStore::new());
        // Bypass the wrapper and write a plain event into the inner store.
        inner
            .append(&Event::new("agg-2", "Test", "Created", 1, json!({ "raw": true })))
            .expect("append raw");
        let store = EncryptedEventStore::new(inner, Arc::new(super::super::XorCipher::new(b"k".to_vec())));
        let result = store.get_events("agg-2");
        assert!(matches!(result, Err(EventError::Encryption(_))));
    }

    #[test]
    fn encrypted_store_get_all_decrypts_every_event() {
        let inner = Arc::new(InMemoryEventStore::new());
        let cipher = Arc::new(super::super::XorCipher::new(b"k".to_vec()));
        let store = EncryptedEventStore::new(inner, cipher);

        store
            .append(&Event::new("agg-3", "Test", "A", 1, json!({ "n": 1 })))
            .expect("append 1");
        store
            .append(&Event::new("agg-3", "Test", "B", 2, json!({ "n": 2 })))
            .expect("append 2");

        let events = store.get_all_events().expect("decrypt all");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].payload, json!({ "n": 1 }));
        assert_eq!(events[1].payload, json!({ "n": 2 }));
    }

    /// Compile-time assertion that the public types are `Send + Sync`.
    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn encrypted_event_store_is_send_and_sync() {
        assert_send_sync::<EncryptedEventStore>();
    }
}
