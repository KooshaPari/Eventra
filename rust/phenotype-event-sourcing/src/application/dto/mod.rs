//! # Data Transfer Objects
//!
//! DTOs for event sourcing operations.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// DTO for event information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDto {
    pub id: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub actor: String,
    pub timestamp: DateTime<Utc>,
    pub sequence: i64,
    pub payload: serde_json::Value,
}

/// DTO for chain verification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainVerificationDto {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub is_valid: bool,
    pub error: Option<String>,
    pub events_verified: i64,
}

/// Command to append a new event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEventDto {
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub actor: String,
    pub payload: serde_json::Value,
}
