//! Common error types for event-bus contract operations.

use derive_more::From;
use thiserror::Error;

/// Result type for event-bus contract operations.
pub type Result<T> = std::result::Result<T, EventBusError>;

/// Errors emitted by event-bus ports.
#[derive(Debug, Error)]
pub enum EventBusError {
    /// Publishing a single event failed.
    #[error("Publish failed: {0}")]
    Publish(String),

    /// Publishing a batch of events failed.
    #[error("Batch publish failed: {0}")]
    BatchPublish(String),

    /// Subscribing a handler failed.
    #[error("Subscribe failed: {0}")]
    Subscribe(String),

    /// Handler dispatch failed.
    #[error("Handler error: {0}")]
    Handler(String),

    /// Event store append failed.
    #[error("Event store append failed: {0}")]
    StoreAppend(String),

    /// Event store read failed.
    #[error("Event store read failed: {0}")]
    StoreRead(String),

    /// Optimistic concurrency conflict.
    #[error("Concurrency conflict: expected version {expected}, found {found}")]
    ConcurrencyConflict {
        /// Expected aggregate version.
        expected: u32,
        /// Observed aggregate version.
        found: u32,
    },

    /// Wraps a store-level error with automatic conversion.
    #[error(transparent)]
    Store(#[from] EventStoreError),
}

/// Event store–level errors.
///
/// `serde_json::Error` and `std::io::Error` get automatic `From` impls via
/// `#[from]`. The two string variants (`AggregateNotFound`, `InvalidInput`)
/// cannot also derive `From<String>` — that would produce conflicting
/// `From<String>` impls — so callers must construct them explicitly.
#[derive(Debug, From)]
pub enum EventStoreError {
    /// JSON serialisation / deserialisation failed.
    #[from]
    SerdeJson(serde_json::Error),
    /// I/O operation failed (disk, network, …).
    #[from]
    Io(std::io::Error),
    /// The requested aggregate was not found.
    AggregateNotFound(String),
    /// A generic invalid‑input error.
    InvalidInput(String),
}

impl From<&str> for EventStoreError {
    fn from(s: &str) -> Self {
        Self::InvalidInput(s.to_string())
    }
}

// ── Display impl (derive_more cannot produce rich enum messages, so we
//    provide a short manual impl instead of pulling in thiserror here). ──

impl std::fmt::Display for EventStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerdeJson(e) => write!(f, "Event store serialization error: {e}"),
            Self::Io(e) => write!(f, "Event store I/O error: {e}"),
            Self::AggregateNotFound(id) => write!(f, "Aggregate not found: {id}"),
            Self::InvalidInput(msg) => write!(f, "Invalid input: {msg}"),
        }
    }
}

impl std::error::Error for EventStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SerdeJson(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::AggregateNotFound(_) | Self::InvalidInput(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_serde_json_error() {
        let serde_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let err: EventStoreError = serde_err.into();
        assert!(matches!(err, EventStoreError::SerdeJson(_)));
        assert!(err.to_string().contains("serialization error"));
    }

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: EventStoreError = io_err.into();
        assert!(matches!(err, EventStoreError::Io(_)));
        assert!(err.to_string().contains("I/O error"));
    }

    #[test]
    fn from_aggregate_not_found() {
        let err = EventStoreError::AggregateNotFound("agg-42".into());
        assert!(err.to_string().contains("agg-42"));
    }

    #[test]
    fn from_invalid_input() {
        let err = EventStoreError::InvalidInput("bad data".into());
        assert!(err.to_string().contains("bad data"));
    }

    #[test]
    fn into_event_bus_error() {
        let store_err = EventStoreError::AggregateNotFound("agg-1".into());
        let bus_err: EventBusError = store_err.into();
        assert!(matches!(bus_err, EventBusError::Store(_)));
    }

    #[test]
    fn display_roundtrip() {
        let err = EventStoreError::InvalidInput("test".into());
        let msg = err.to_string();
        assert!(msg.contains("test"));
    }
}
