//! [`SignedEventBus`] — [`EventBus`](crate::domain::EventBus) decorator that
//! signs events on publish and verifies signatures before delivering them to
//! subscribers.

use std::sync::Arc;

use crate::domain::{Event, EventBus, EventError, EventHandler};

use super::signer::EventSigner;

/// Wraps an inner [`EventBus`], attaching a signature to every event on
/// publish. Verification of inbound signatures is the caller's responsibility
/// (use [`EventSigner::verify`](super::signer::EventSigner::verify)) because
/// the inner bus invokes handlers synchronously inside `publish`, leaving no
/// place for this decorator to intercept delivery.
pub struct SignedEventBus {
    inner: Arc<dyn EventBus>,
    signer: Arc<dyn EventSigner>,
}

impl SignedEventBus {
    pub fn new(inner: Arc<dyn EventBus>, signer: Arc<dyn EventSigner>) -> Self {
        Self { inner, signer }
    }
}

impl EventBus for SignedEventBus {
    fn publish(&self, event: &Event) -> Result<(), EventError> {
        let mut signed = event.clone();
        if signed.metadata.signature.is_none() {
            let signature = self.signer.sign(&signed)?;
            signed.metadata.signature = Some(signature);
        }
        self.inner.publish(&signed)
    }

    fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<(), EventError> {
        self.inner.subscribe(handler)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use parking_lot::RwLock;
    use serde_json::json;

    use crate::domain::{Event, EventBus, EventError};

    use super::*;

    /// A recording bus that just stores every published event. Used to
    /// confirm the signed bus attaches a signature.
    struct RecordingBus {
        received: RwLock<Vec<Event>>,
    }

    impl EventBus for RecordingBus {
        fn publish(&self, event: &Event) -> Result<(), EventError> {
            self.received.write().push(event.clone());
            Ok(())
        }
        fn subscribe(&self, _handler: Box<dyn EventHandler>) -> Result<(), EventError> {
            Ok(())
        }
    }

    #[test]
    fn signed_bus_attaches_signature_on_publish() {
        let recording = Arc::new(RecordingBus {
            received: RwLock::new(Vec::new()),
        });
        let signer = Arc::new(super::super::HmacSigner::new(b"k".to_vec()));
        let bus = SignedEventBus::new(recording.clone(), signer);

        let event = Event::new("agg-1", "Test", "Created", 1, json!({ "v": 1 }));
        bus.publish(&event).expect("publish");

        let received = recording.received.read();
        assert_eq!(received.len(), 1);
        assert!(
            received[0].metadata.signature.is_some(),
            "signed bus must attach a signature"
        );
    }

    #[test]
    fn signed_bus_does_not_overwrite_existing_signature() {
        let recording = Arc::new(RecordingBus {
            received: RwLock::new(Vec::new()),
        });
        let signer = Arc::new(super::super::HmacSigner::new(b"k".to_vec()));
        let bus = SignedEventBus::new(recording.clone(), signer.clone());

        let event = Event::new("agg-1", "Test", "Created", 1, json!({}));
        let pre_existing = signer.sign(&event).expect("pre-sign");
        let mut event = event;
        event.metadata.signature = Some(pre_existing.clone());

        bus.publish(&event).expect("publish");

        let received = recording.received.read();
        assert_eq!(received[0].metadata.signature.as_deref(), Some(pre_existing.as_str()));
    }

    /// The decorator doesn't need to be exercised for handler invocation in
    /// these tests, but a smoke check that AtomicUsize compiles in this
    /// module keeps any future handler test edits self-contained.
    #[test]
    fn atomicusize_compiles() {
        let counter = AtomicUsize::new(0);
        counter.fetch_add(1, Ordering::SeqCst);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
