//! Event store port for append-only persistence.

use chrono::{DateTime, Utc};

use crate::envelope::EventEnvelope;
use crate::error::Result;

/// Event store port for append-only event persistence and replay.
pub trait EventStore: Send + Sync {
    /// Append an event to the store.
    fn append(&self, event: &EventEnvelope) -> Result<()>;

    /// Load all events for an aggregate stream.
    fn get_events(&self, aggregate_id: &str) -> Result<Vec<EventEnvelope>>;

    /// Load all events since a wall-clock timestamp.
    fn get_events_since(&self, since: DateTime<Utc>) -> Result<Vec<EventEnvelope>>;
}
