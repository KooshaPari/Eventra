use async_trait::async_trait;
use phenotype_error_core::{RepositoryError, StorageError};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use thiserror::Error;
use ulid::Ulid;

pub mod outbox;
pub use outbox::{InMemoryOutbox, OutboxEntry, OutboxError, OutboxStore};

pub mod outbox_relay;
pub use outbox_relay::{
    new_shutdown_token, run as run_outbox_relay, spawn_workers as spawn_outbox_relay_workers,
    Publisher as OutboxPublisher, RelayConfig, RelayStats, Shutdown as OutboxShutdown,
};

#[cfg(feature = "postgres")]
pub use outbox::postgres as outbox_postgres;

/// Event ID using ULID for sortability
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
pub struct EventId(Ulid);

impl EventId {
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    pub fn as_ulid(&self) -> Ulid {
        self.0
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Event envelope with metadata
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct EventEnvelope<T: Clone> {
    pub id: EventId,
    pub timestamp: u64,
    pub source: String,
    pub payload: T,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
}

impl<T: Clone> EventEnvelope<T> {
    pub fn new(source: impl Into<String>, payload: T) -> Self {
        Self {
            id: EventId::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            source: source.into(),
            payload,
            correlation_id: None,
            causation_id: None,
        }
    }

    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    pub fn with_causation_id(mut self, id: impl Into<String>) -> Self {
        self.causation_id = Some(id.into());
        self
    }
}

/// Event bus errors
#[derive(Error, Debug)]
pub enum EventBusError {
    #[error("Publish error: {0}")]
    Publish(String),
    #[error("Subscribe error: {0}")]
    Subscribe(String),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Timeout")]
    Timeout,
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// Event bus trait for pluggable backends
#[async_trait]
pub trait EventBus: Send + Sync + 'static {
    type Event: Serialize + DeserializeOwned + Send + Sync + Clone + Debug + 'static;

    /// Publish an event to the bus
    async fn publish(&self, event: EventEnvelope<Self::Event>) -> Result<(), EventBusError>;

    /// Subscribe to events matching a pattern
    async fn subscribe<F>(&self, subject: &str, handler: F) -> Result<Subscription, EventBusError>
    where
        F: Fn(EventEnvelope<Self::Event>) -> Result<(), EventBusError> + Send + Sync + 'static;

    /// Request-response pattern
    async fn request<T: Serialize + DeserializeOwned + Send + Sync + Clone + Debug + 'static>(
        &self,
        subject: &str,
        payload: T,
        timeout_ms: u64,
    ) -> Result<EventEnvelope<T>, EventBusError>;

    /// Close the bus connection
    async fn close(&self) -> Result<(), EventBusError>;
}

/// Subscription handle
#[derive(Debug)]
pub struct Subscription {
    pub id: String,
    pub subject: String,
}

impl Subscription {
    pub fn new(id: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            subject: subject.into(),
        }
    }
}

/// In-memory event bus implementation for testing
pub mod memory;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq)]
    struct TestEvent {
        data: String,
    }

    #[test]
    fn fr_event_bus_001_event_id_generation() {
        let id1 = EventId::new();
        let id2 = EventId::new();
        assert_ne!(id1, id2, "Each EventId should be unique");
    }

    #[test]
    fn fr_event_bus_002_event_envelope_creation() {
        let event = TestEvent {
            data: "test".to_string(),
        };
        let envelope = EventEnvelope::new("test_source", event);

        assert_eq!(envelope.source, "test_source");
        assert_eq!(envelope.payload.data, "test");
        assert!(envelope.correlation_id.is_none());
    }

    #[test]
    fn fr_event_bus_003_event_envelope_with_correlation() {
        let event = TestEvent {
            data: "test".to_string(),
        };
        let envelope = EventEnvelope::new("test_source", event)
            .with_correlation_id("cor-123")
            .with_causation_id("cause-456");

        assert_eq!(envelope.correlation_id, Some("cor-123".to_string()));
        assert_eq!(envelope.causation_id, Some("cause-456".to_string()));
    }
}
