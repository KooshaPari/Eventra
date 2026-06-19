//! # Outbound Ports (Secondary/Driven Ports)
//!
//! These ports define the **capabilities** that infrastructure must provide.
//! The domain defines these traits; adapters implement them.
//!
//! ## Common Outbound Ports
//!
//! | Port | Purpose |
//! |------|---------|
//! | `EventRepository` | Persist and retrieve events |
//! | `SnapshotStore` | Store/retrieve aggregate snapshots |
//!
//! ## Example
//!
//! ```rust,ignore
//! use crate::domain::ports::outbound::EventRepository;
//! use crate::domain::entities::EventEnvelope;
//!
//! // Outbound port trait (defined by domain)
//! pub trait EventRepository: Send + Sync {
//!     fn append<T: Serialize>(&self, event: &EventEnvelope<T>, agg_type: &str) -> Result<i64>;
//!     fn get_events<T: Serialize>(&self, agg_type: &str, agg_id: &str) -> Result<Vec<EventEnvelope<T>>>;
//! }
//!
//! // Adapter implements the port
//! pub struct SqlxEventRepository { pool: PgPool }
//! impl EventRepository for SqlxEventRepository { ... }
//! ```

use super::super::entities::EventEnvelope;
use chrono::{DateTime, Utc};

/// Outbound port for event persistence.
///
/// This is the primary interface for storing and retrieving events.
pub trait EventRepository: Send + Sync {
    /// Append a new event; returns the assigned sequence number.
    fn append<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(
        &self,
        event: &EventEnvelope<T>,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<i64, RepositoryError>;

    /// Get all events for a given aggregate, in ascending sequence order.
    fn get_events<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Vec<EventEnvelope<T>>, RepositoryError>;

    /// Get events from a specific sequence onward (exclusive).
    fn get_events_since<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        sequence: i64,
    ) -> Result<Vec<EventEnvelope<T>>, RepositoryError>;

    /// Get events within a time range (inclusive).
    fn get_events_by_range<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<EventEnvelope<T>>, RepositoryError>;

    /// Get the latest event sequence number for an aggregate (0 if none exist).
    fn get_latest_sequence(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<i64, RepositoryError>;
}

/// Outbound port for aggregate snapshot storage.
pub trait SnapshotRepository: Send + Sync {
    /// Save a snapshot of an aggregate.
    fn save_snapshot<T: serde::Serialize>(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        sequence: i64,
        state: &T,
    ) -> Result<(), RepositoryError>;

    /// Load the latest snapshot for an aggregate.
    fn load_snapshot<T: serde::de::DeserializeOwned>(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Option<(i64, T)>, RepositoryError>;

    /// Delete snapshots older than a sequence.
    fn delete_old_snapshots(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        keep_after_sequence: i64,
    ) -> Result<(), RepositoryError>;
}

/// Errors that can occur in repositories.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Event not found: {0}")]
    NotFound(String),

    #[error("Duplicate sequence: {0}")]
    DuplicateSequence(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Invalid hash: {0}")]
    InvalidHash(String),

    #[error("Sequence gap: expected {expected}, got {actual}")]
    SequenceGap { expected: i64, actual: i64 },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Re-export for convenience.
pub use super::super::super::error::EventSourcingError;
