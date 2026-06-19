//! # Application Use Cases
//!
//! Use cases orchestrate the domain logic.

use crate::domain::entities::EventEnvelope;
use crate::domain::ports::outbound::{EventRepository, RepositoryError};
use crate::domain::services::{compute_event_hash, validate_hash};

/// Error type for use case operations.
#[derive(Debug, thiserror::Error)]
pub enum UseCaseError {
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid event: {0}")]
    InvalidEvent(String),

    #[error("Chain verification failed: {0}")]
    ChainVerification(String),
}

/// Use case for appending an event to an aggregate.
pub struct AppendEventUseCase<R: EventRepository> {
    repository: R,
}

impl<R: EventRepository> AppendEventUseCase<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn execute<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        mut event: EventEnvelope<T>,
        event_type: &str,
    ) -> Result<i64, UseCaseError> {
        // Get the latest sequence for this aggregate
        let prev_sequence = self
            .repository
            .get_latest_sequence(aggregate_type, aggregate_id)?;

        // Set the previous hash
        if prev_sequence == 0 {
            event.prev_hash = "0".repeat(64);
        } else {
            // Get the previous event to get its hash
            let events = self.repository.get_events::<serde_json::Value>(aggregate_type, aggregate_id)?;
            if let Some(last_event) = events.last() {
                event.prev_hash = last_event.hash.clone();
            }
        }

        // Compute the event hash
        let payload_bytes = serde_json::to_vec(&event.payload)?;
        let hash = compute_event_hash(
            &event.prev_hash,
            &event.id.to_string(),
            &event.timestamp.to_rfc3339(),
            &event.actor,
            &payload_bytes,
        );
        event.hash = hash;

        // Validate the hash
        validate_hash(&event.hash).map_err(|e| UseCaseError::InvalidEvent(e.to_string()))?;

        // Append to repository
        let sequence = self.repository.append(&event, aggregate_type, aggregate_id)?;
        Ok(sequence)
    }
}

/// Use case for getting events for an aggregate.
pub struct GetEventsUseCase<R: EventRepository> {
    repository: R,
}

impl<R: EventRepository> GetEventsUseCase<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn execute<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Vec<EventEnvelope<T>>, UseCaseError> {
        let events = self.repository.get_events(aggregate_type, aggregate_id)?;
        Ok(events)
    }

    pub fn execute_since<T: serde::Serialize + for<'de> serde::Deserialize<'de>>(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        sequence: i64,
    ) -> Result<Vec<EventEnvelope<T>>, UseCaseError> {
        let events = self.repository.get_events_since(aggregate_type, aggregate_id, sequence)?;
        Ok(events)
    }
}

/// Use case for verifying hash chain integrity.
pub struct VerifyChainUseCase<R: EventRepository> {
    repository: R,
}

impl<R: EventRepository> VerifyChainUseCase<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn execute(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<bool, UseCaseError> {
        // Get all events
        let events: Vec<EventEnvelope<serde_json::Value>> =
            self.repository.get_events(aggregate_type, aggregate_id)?;

        if events.is_empty() {
            return Ok(true); // Empty stream is valid
        }

        // Verify each event's hash
        for event in &events {
            validate_hash(&event.hash)
                .map_err(|e| UseCaseError::ChainVerification(e.to_string()))?;
        }

        Ok(true)
    }
}
