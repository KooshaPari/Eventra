//! # In-Memory Event Repository
//!
//! A simple in-memory implementation of the EventRepository port.
//! Useful for testing and prototyping.

use std::collections::HashMap;
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::entities::EventEnvelope;
use crate::domain::ports::outbound::{EventRepository, RepositoryError};
use crate::domain::services::compute_event_hash;

/// In-memory event store implementation.
pub struct InMemoryEventStore {
    /// Events keyed by (aggregate_type, aggregate_id, sequence)
    events: RwLock<HashMap<(String, String, i64), InMemoryEvent>>,
    /// Latest sequence per aggregate
    latest_sequences: RwLock<HashMap<(String, String), i64>>,
}

/// Internal event storage format.
#[derive(Debug, Clone)]
struct InMemoryEvent {
    pub id: uuid::Uuid,
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub event_type: String,
    pub payload: Vec<u8>,
    pub prev_hash: String,
    pub hash: String,
    pub sequence: i64,
}

impl InMemoryEventStore {
    /// Create a new in-memory event store.
    pub fn new() -> Self {
        Self {
            events: RwLock::new(HashMap::new()),
            latest_sequences: RwLock::new(HashMap::new()),
        }
    }

    /// Get the number of events stored.
    pub fn event_count(&self) -> usize {
        self.events.read().unwrap().len()
    }

    /// Clear all events (useful for testing).
    pub fn clear(&self) {
        self.events.write().unwrap().clear();
        self.latest_sequences.write().unwrap().clear();
    }
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EventRepository for InMemoryEventStore {
    fn append<T: Serialize + for<'de> Deserialize<'de>>(
        &self,
        event: &EventEnvelope<T>,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<i64, RepositoryError> {
        let mut latest = self.latest_sequences.write().unwrap();
        let current_seq = latest.get(&(aggregate_type.to_string(), aggregate_id.to_string())).copied().unwrap_or(0);
        let next_seq = current_seq + 1;

        // Serialize payload
        let payload_bytes = serde_json::to_vec(&event.payload)?;

        // Compute hash
        let hash = compute_event_hash(
            &event.prev_hash,
            &event.id.to_string(),
            &event.timestamp.to_rfc3339(),
            &event.actor,
            &payload_bytes,
        );

        // Store event
        let inmemory_event = InMemoryEvent {
            id: event.id,
            timestamp: event.timestamp,
            actor: event.actor.clone(),
            event_type: aggregate_type.to_string(),
            payload: payload_bytes,
            prev_hash: event.prev_hash.clone(),
            hash: hash.clone(),
            sequence: next_seq,
        };

        self.events.write().unwrap().insert(
            (aggregate_type.to_string(), aggregate_id.to_string(), next_seq),
            inmemory_event,
        );

        latest.insert((aggregate_type.to_string(), aggregate_id.to_string()), next_seq);

        Ok(next_seq)
    }

    fn get_events<T: Serialize + for<'de> Deserialize<'de>>(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Vec<EventEnvelope<T>>, RepositoryError> {
        let events = self.events.read().unwrap();
        let key = (aggregate_type.to_string(), aggregate_id.to_string());

        let mut result: Vec<EventEnvelope<T>> = Vec::new();

        // Collect all events for this aggregate
        let mut seqs: Vec<i64> = events
            .keys()
            .filter(|(t, i, _)| t == aggregate_type && i == aggregate_id)
            .map(|(_, _, s)| *s)
            .collect();
        seqs.sort();

        for seq in seqs {
            if let Some(event) = events.get(&(aggregate_type.to_string(), aggregate_id.to_string(), seq)) {
                let payload: T = serde_json::from_slice(&event.payload)?;
                result.push(EventEnvelope {
                    id: event.id,
                    timestamp: event.timestamp,
                    payload,
                    actor: event.actor.clone(),
                    prev_hash: event.prev_hash.clone(),
                    hash: event.hash.clone(),
                    sequence: event.sequence,
                });
            }
        }

        Ok(result)
    }

    fn get_events_since<T: Serialize + for<'de> Deserialize<'de>>(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        sequence: i64,
    ) -> Result<Vec<EventEnvelope<T>>, RepositoryError> {
        let events = self.events.read().unwrap();
        let key = (aggregate_type.to_string(), aggregate_id.to_string());

        let mut result: Vec<EventEnvelope<T>> = Vec::new();

        // Collect events with sequence > given sequence
        let mut seqs: Vec<i64> = events
            .keys()
            .filter(|(t, i, s)| t == aggregate_type && i == aggregate_id && *s > sequence)
            .map(|(_, _, s)| *s)
            .collect();
        seqs.sort();

        for seq in seqs {
            if let Some(event) = events.get(&(aggregate_type.to_string(), aggregate_id.to_string(), seq)) {
                let payload: T = serde_json::from_slice(&event.payload)?;
                result.push(EventEnvelope {
                    id: event.id,
                    timestamp: event.timestamp,
                    payload,
                    actor: event.actor.clone(),
                    prev_hash: event.prev_hash.clone(),
                    hash: event.hash.clone(),
                    sequence: event.sequence,
                });
            }
        }

        Ok(result)
    }

    fn get_events_by_range<T: Serialize + for<'de> Deserialize<'de>>(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<EventEnvelope<T>>, RepositoryError> {
        let events = self.events.read().unwrap();
        let key = (aggregate_type.to_string(), aggregate_id.to_string());

        let mut result: Vec<EventEnvelope<T>> = Vec::new();

        // Collect events within time range
        let mut seqs: Vec<i64> = events
            .keys()
            .filter(|(t, i, _)| t == aggregate_type && i == aggregate_id)
            .map(|(_, _, s)| *s)
            .collect();
        seqs.sort();

        for seq in seqs {
            if let Some(event) = events.get(&(aggregate_type.to_string(), aggregate_id.to_string(), seq)) {
                if event.timestamp >= from && event.timestamp <= to {
                    let payload: T = serde_json::from_slice(&event.payload)?;
                    result.push(EventEnvelope {
                        id: event.id,
                        timestamp: event.timestamp,
                        payload,
                        actor: event.actor.clone(),
                        prev_hash: event.prev_hash.clone(),
                        hash: event.hash.clone(),
                        sequence: event.sequence,
                    });
                }
            }
        }

        Ok(result)
    }

    fn get_latest_sequence(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<i64, RepositoryError> {
        let latest = self.latest_sequences.read().unwrap();
        Ok(latest.get(&(aggregate_type.to_string(), aggregate_id.to_string())).copied().unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_retrieve_event() {
        let store = InMemoryEventStore::new();

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct TestPayload {
            value: String,
        }

        let event = EventEnvelope::new(TestPayload { value: "test".to_string() }, "user-1");
        let seq = store.append(&event, "User", "user-123").unwrap();

        assert_eq!(seq, 1);

        let retrieved: Vec<EventEnvelope<TestPayload>> = store.get_events("User", "user-123").unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].payload.value, "test");
    }

    #[test]
    fn sequence_increments() {
        let store = InMemoryEventStore::new();

        #[derive(Serialize, Deserialize)]
        struct Event;
        
        for i in 0..5 {
            let event = EventEnvelope::new(Event, "user");
            let seq = store.append(&event, "Test", "id").unwrap();
            assert_eq!(seq, i + 1);
        }
    }

    #[test]
    fn get_events_since() {
        let store = InMemoryEventStore::new();

        #[derive(Serialize, Deserialize)]
        struct Event;
        
        for _ in 0..5 {
            let event = EventEnvelope::new(Event, "user");
            store.append(&event, "Test", "id").unwrap();
        }

        let events: Vec<EventEnvelope<Event>> = store.get_events_since("Test", "id", 3).unwrap();
        assert_eq!(events.len(), 2);
    }
}
