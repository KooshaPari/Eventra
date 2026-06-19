//! # Entities
//!
//! Core domain entities for event sourcing.
//!
//! ## Main Entities
//!
//! - [`EventEnvelope`] - Generic event wrapper with hash chain support

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Generic event envelope that works with any serializable event type.
///
/// The event is stored in a hash chain where each event includes:
/// - A unique ID
/// - A timestamp
/// - An event type discriminator (as a string)
/// - The serialized event payload
/// - The hash of the previous event in the chain
/// - The hash of this event (computed by the store)
/// - A monotonically increasing sequence number
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope<T: Serialize> {
    /// Unique event ID
    pub id: Uuid,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// The actual event payload (generic type)
    pub payload: T,

    /// Actor who triggered this event (e.g., user ID, system name)
    pub actor: String,

    /// Hash of the previous event in the chain (32 bytes, hex-encoded for storage)
    pub prev_hash: String,

    /// Hash of this event (computed by store, hex-encoded, 32 bytes)
    pub hash: String,

    /// Monotonically increasing sequence number (per entity)
    pub sequence: i64,
}

impl<T: Serialize> EventEnvelope<T> {
    /// Create a new event envelope.
    /// Hash and sequence will be assigned by the store.
    pub fn new(payload: T, actor: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            payload,
            actor: actor.into(),
            prev_hash: "0".repeat(64), // Initial zero hash (32 bytes = 64 hex chars)
            hash: "".to_string(),       // Will be filled by store
            sequence: 0,                // Will be filled by store
        }
    }

    /// Create an event envelope with a specific previous hash.
    pub fn with_prev_hash(payload: T, actor: impl Into<String>, prev_hash: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            payload,
            actor: actor.into(),
            prev_hash,
            hash: "".to_string(),
            sequence: 0,
        }
    }

    /// Check if this is the first event in a chain (has zero prev_hash).
    pub fn is_first_event(&self) -> bool {
        self.prev_hash == "0".repeat(64)
    }
}

/// Errors for entity operations.
#[derive(Debug, thiserror::Error)]
pub enum EntityError {
    #[error("Invalid sequence: {0}")]
    InvalidSequence(String),

    #[error("Hash mismatch: {0}")]
    HashMismatch(String),
}
