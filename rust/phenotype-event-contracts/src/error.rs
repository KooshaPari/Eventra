//! Common error types for event-bus contract operations.

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
}
