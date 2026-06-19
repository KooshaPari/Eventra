//! Event envelope types shared by pub/sub and store ports.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Metadata attached to every domain event envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventMetadata {
    /// Unique event identifier.
    pub event_id: Uuid,
    /// Aggregate instance identifier.
    pub aggregate_id: String,
    /// Aggregate type name.
    pub aggregate_type: String,
    /// Event type discriminator.
    pub event_type: String,
    /// Monotonic aggregate version.
    pub version: u32,
    /// Wall-clock timestamp (UTC).
    pub timestamp: DateTime<Utc>,
    /// Optional causation chain identifier.
    pub causation_id: Option<Uuid>,
    /// Optional correlation identifier for tracing.
    pub correlation_id: Option<Uuid>,
}

impl EventMetadata {
    /// Construct metadata for a new event.
    pub fn new(
        aggregate_id: impl Into<String>,
        aggregate_type: impl Into<String>,
        event_type: impl Into<String>,
        version: u32,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            aggregate_id: aggregate_id.into(),
            aggregate_type: aggregate_type.into(),
            event_type: event_type.into(),
            version,
            timestamp: Utc::now(),
            causation_id: None,
            correlation_id: None,
        }
    }
}

/// Serializable domain event envelope used by pub/sub and store ports.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventEnvelope {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Opaque JSON payload.
    pub payload: serde_json::Value,
}

impl EventEnvelope {
    /// Construct a new event envelope.
    pub fn new(
        aggregate_id: impl Into<String>,
        aggregate_type: impl Into<String>,
        event_type: impl Into<String>,
        version: u32,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            metadata: EventMetadata::new(aggregate_id, aggregate_type, event_type, version),
            payload,
        }
    }

    /// Attach a causation identifier.
    pub fn with_causation_id(mut self, id: Uuid) -> Self {
        self.metadata.causation_id = Some(id);
        self
    }

    /// Attach a correlation identifier.
    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.metadata.correlation_id = Some(id);
        self
    }
}
