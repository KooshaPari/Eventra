//! # Inbound Ports (Primary/Driving Ports)
//!
//! These ports define the **use cases** that the application exposes.
//! Inbound adapters (API, CLI) call these ports to trigger business logic.
//!
//! ## Example Use Cases
//!
//! - `AppendEventPort` - Append a new event to an aggregate
//! - `GetEventsPort` - Retrieve events for an aggregate
//! - `VerifyChainPort` - Verify hash chain integrity

use super::super::entities::EventEnvelope;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Command to append a new event.
pub struct AppendEventCommand<T: serde::Serialize + for<'de> serde::Deserialize<'de>> {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event: EventEnvelope<T>,
}

/// Query to get events for an aggregate.
pub struct GetEventsQuery {
    pub aggregate_type: String,
    pub aggregate_id: String,
}

/// Query to get events since a specific sequence.
pub struct GetEventsSinceQuery {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub sequence: i64,
}

/// Query to get events within a time range.
pub struct GetEventsByTimeRangeQuery {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

/// Inbound port for event operations.
pub trait EventCommandPort<T: serde::Serialize + for<'de> serde::Deserialize<'de>>: Send + Sync {
    /// Append a new event to an aggregate.
    fn append_event(
        &self,
        cmd: AppendEventCommand<T>,
    ) -> Result<i64, EventSourcingError>;

    /// Get all events for an aggregate.
    fn get_events(
        &self,
        query: GetEventsQuery,
    ) -> Result<Vec<EventEnvelope<T>>, EventSourcingError>;

    /// Get events since a specific sequence.
    fn get_events_since(
        &self,
        query: GetEventsSinceQuery,
    ) -> Result<Vec<EventEnvelope<T>>, EventSourcingError>;

    /// Get events within a time range.
    fn get_events_by_time_range(
        &self,
        query: GetEventsByTimeRangeQuery,
    ) -> Result<Vec<EventEnvelope<T>>, EventSourcingError>;
}

/// Inbound port for chain verification.
pub trait ChainVerificationPort: Send + Sync {
    /// Verify the hash chain integrity for an aggregate.
    fn verify_chain(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<(), EventSourcingError>;

    /// Get the latest sequence number for an aggregate.
    fn get_latest_sequence(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<i64, EventSourcingError>;
}

/// Re-export common error types.
pub use super::super::super::error::EventSourcingError;
