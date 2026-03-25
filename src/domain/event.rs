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
pub trait EventHandler: Send + Sync {
    fn handle(&self, event: &Event) -> Result<(), EventError>;
    fn event_types(&self) -> Vec<String>;
}

/// Event store trait - secondary port
pub trait EventStore: Send + Sync {
    fn append(&self, event: &Event) -> Result<(), EventError>;
    fn get_events(&self, aggregate_id: &str) -> Result<Vec<Event>, EventError>;
    fn get_events_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, EventError>;
}
