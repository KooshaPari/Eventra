//! Event Store Adapters

use std::collections::HashMap;
use parking_lot::RwLock;
use chrono::{DateTime, Utc};
use async_trait::async_trait;

use crate::domain::{Event, EventStore, EventError};

/// In-memory event store adapter
pub struct InMemoryEventStore {
    events: RwLock<HashMap<String, Vec<Event>>>,
    all_events: RwLock<Vec<Event>>,
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        Self {
            events: RwLock::new(HashMap::new()),
            all_events: RwLock::new(Vec::new()),
        }
    }
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStore for InMemoryEventStore {
    fn append(&self, event: &Event) -> Result<(), EventError> {
        let mut events = self.events.write();
        let aggregate_events = events.entry(event.metadata.aggregate_id.clone())
            .or_insert_with(Vec::new);

        // Check version
        if let Some(last) = aggregate_events.last() {
            if last.metadata.version != event.metadata.version - 1 {
                return Err(EventError::ConcurrencyConflict {
                    expected: last.metadata.version + 1,
                    found: event.metadata.version,
                });
            }
        }

        aggregate_events.push(event.clone());
        self.all_events.write().push(event.clone());

        Ok(())
    }

    fn get_events(&self, aggregate_id: &str) -> Result<Vec<Event>, EventError> {
        let events = self.events.read();
        Ok(events.get(aggregate_id).cloned().unwrap_or_default())
    }

    fn get_events_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, EventError> {
        let all = self.all_events.read();
        Ok(all.iter()
            .filter(|e| e.metadata.timestamp > since)
            .cloned()
            .collect())
    }
}
