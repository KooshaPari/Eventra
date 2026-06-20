//! Event - Domain Entity

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::error::EventError;

/// Event metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub event_id: Uuid,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub event_type: String,
    pub version: u32,
    pub timestamp: DateTime<Utc>,
    pub causation_id: Option<Uuid>,
    pub correlation_id: Option<Uuid>,
    /// Optional integrity signature. Populated by an
    /// [`EventSigner`](crate::security::EventSigner) and verified on read.
    /// Excluded from the canonical signed payload (see
    /// [`crate::security::signer`]) so that signing and verification operate
    /// over the same byte sequence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl EventMetadata {
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
            signature: None,
        }
    }
}

/// Domain event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub metadata: EventMetadata,
    pub payload: serde_json::Value,
}

impl Event {
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

    pub fn with_causation_id(mut self, id: Uuid) -> Self {
        self.metadata.causation_id = Some(id);
        self
    }

    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.metadata.correlation_id = Some(id);
        self
    }
}

/// Event bus trait - primary port
pub trait EventBus: Send + Sync {
    fn publish(&self, event: &Event) -> Result<(), EventError>;
    fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<(), EventError>;
}

/// Event handler trait
pub trait EventHandler: Send + Sync + EventHandlerClone {
    fn handle(&self, event: &Event) -> Result<(), EventError>;
    fn event_types(&self) -> Vec<String>;
}

/// Cloning helper for `EventHandler` trait objects. Supertrait of
/// [`EventHandler`] so that `dyn EventHandler` values can be cloned
/// through a single virtual call.
pub trait EventHandlerClone {
    fn clone_boxed(&self) -> Box<dyn EventHandler>;
}

/// Event store trait - secondary port
pub trait EventStore: Send + Sync {
    fn append(&self, event: &Event) -> Result<(), EventError>;
    fn get_events(&self, aggregate_id: &str) -> Result<Vec<Event>, EventError>;
    fn get_events_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, EventError>;
    /// Return every stored event in append order. Used by projection replay
    /// so that [`crate::application::projection::ProjectionRunner::run`] can
    /// rebuild projection state from offset 0 (or from a saved position).
    fn get_all_events(&self) -> Result<Vec<Event>, EventError>;
}

/// Maximum permitted size of an event or command payload, in bytes. Payloads
/// exceeding this bound are rejected by the default [`Validate`] implementations
/// to defend against resource exhaustion from untrusted input.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

/// Pluggable validation hook for events, commands, and aggregate state.
///
/// Implementations should return [`EventError::Validation`] for any input that
/// does not satisfy their invariants. The framework runs these checks before
/// persisting or publishing an event, before dispatching a command, and before
/// applying an event to an aggregate.
pub trait Validate {
    fn validate(&self) -> Result<(), EventError>;
}

fn payload_size(value: &serde_json::Value) -> usize {
    serde_json::to_vec(value).map(|v| v.len()).unwrap_or(0)
}

fn require_non_empty(field: &str, value: &str) -> Result<(), EventError> {
    if value.trim().is_empty() {
        return Err(EventError::Validation(format!("{} must not be empty", field)));
    }
    Ok(())
}

fn require_size_limit(field: &str, size: usize) -> Result<(), EventError> {
    if size > MAX_PAYLOAD_BYTES {
        return Err(EventError::Validation(format!(
            "{} exceeds maximum size of {} bytes",
            field, MAX_PAYLOAD_BYTES
        )));
    }
    Ok(())
}

impl Validate for Event {
    fn validate(&self) -> Result<(), EventError> {
        require_non_empty("aggregate_id", &self.metadata.aggregate_id)?;
        require_non_empty("aggregate_type", &self.metadata.aggregate_type)?;
        require_non_empty("event_type", &self.metadata.event_type)?;
        require_size_limit("payload", payload_size(&self.payload))?;
        Ok(())
    }
}

impl Validate for EventMetadata {
    fn validate(&self) -> Result<(), EventError> {
        require_non_empty("aggregate_id", &self.aggregate_id)?;
        require_non_empty("aggregate_type", &self.aggregate_type)?;
        require_non_empty("event_type", &self.event_type)?;
        if self.version == 0 {
            return Err(EventError::Validation("version must be >= 1".into()));
        }
        Ok(())
    }
}
