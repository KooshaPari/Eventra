//! Event Store Adapters

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;

use crate::domain::{Event, EventError, EventStore};

/// Maximum allowed length (in chars) for aggregate identifiers and event
/// types. Defends against memory-exhaustion via pathologically large
/// strings: an attacker that controls `Command::aggregate_id` or
/// `EventMetadata::event_type` could otherwise blow up the per-aggregate
/// `HashMap` key set or the `event_bus` subscriber registration table.
pub const MAX_ID_LENGTH: usize = 256;

/// In-memory event store adapter.
///
/// **PoC / test-only.** This adapter stores events in process memory and
/// loses all data on restart. It is intended for unit tests, PoC code, and
/// local examples. For production use, choose a durable adapter backed by
/// PostgreSQL or SQLite via the feature flags on `phenotype-event-bus`.
#[doc(alias = "test-only")]
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

/// Validate that an event is well-formed before it touches storage.
///
/// Returns `Err(EventError::Store(...))` if the event has an empty
/// aggregate id, an empty event type, or oversized string fields.
fn validate_event(event: &Event) -> Result<(), EventError> {
    let m = &event.metadata;
    if m.aggregate_id.is_empty() {
        return Err(EventError::Store("aggregate_id must not be empty".into()));
    }
    if m.aggregate_type.is_empty() {
        return Err(EventError::Store("aggregate_type must not be empty".into()));
    }
    if m.event_type.is_empty() {
        return Err(EventError::Store("event_type must not be empty".into()));
    }
    if m.aggregate_id.len() > MAX_ID_LENGTH
        || m.aggregate_type.len() > MAX_ID_LENGTH
        || m.event_type.len() > MAX_ID_LENGTH
    {
        return Err(EventError::Store(format!(
            "identifier exceeds MAX_ID_LENGTH ({MAX_ID_LENGTH})"
        )));
    }
    Ok(())
}

impl EventStore for InMemoryEventStore {
    fn append(&self, event: &Event) -> Result<(), EventError> {
        validate_event(event)?;

        let mut events = self.events.write();
        let aggregate_events = events
            .entry(event.metadata.aggregate_id.clone())
            .or_default();

        // Check version.
        //
        // Versions are 1-indexed: the first event for an aggregate has
        // version 1, the next is 2, etc. We reject version 0 outright
        // (it would underflow `version - 1` below and is not a legal
        // value under the documented convention).
        if event.metadata.version == 0 {
            return Err(EventError::ConcurrencyConflict {
                expected: 1,
                found: 0,
            });
        }
        if let Some(last) = aggregate_events.last() {
            // `last.metadata.version + 1` is safe because both sides are
            // bounded `u32`s and the addition only overflows when the
            // last event already has version `u32::MAX`, in which case
            // no further event should ever be appended.
            let expected = last.metadata.version.saturating_add(1);
            if event.metadata.version != expected {
                return Err(EventError::ConcurrencyConflict {
                    expected,
                    found: event.metadata.version,
                });
            }
        } else if event.metadata.version != 1 {
            // First event for this aggregate must be version 1.
            return Err(EventError::ConcurrencyConflict {
                expected: 1,
                found: event.metadata.version,
            });
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
        Ok(all
            .iter()
            .filter(|e| e.metadata.timestamp > since)
            .cloned()
            .collect())
    }

    fn get_all_events(&self) -> Result<Vec<Event>, EventError> {
        Ok(self.all_events.read().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::EventMetadata;
    use serde_json::json;

    fn event(aggregate_id: &str, event_type: &str, version: u32) -> Event {
        Event {
            metadata: EventMetadata::new(aggregate_id, "TestAggregate", event_type, version),
            payload: json!({}),
        }
    }

    #[test]
    fn append_first_event_must_be_version_one() {
        let store = InMemoryEventStore::new();
        // version 0 is rejected outright (prevents u32 underflow in the
        // legacy `version - 1` arithmetic and matches the 1-indexed
        // convention).
        let err = store.append(&event("agg-1", "Created", 0)).unwrap_err();
        assert!(
            matches!(
                err,
                EventError::ConcurrencyConflict {
                    expected: 1,
                    found: 0
                }
            ),
            "expected ConcurrencyConflict {{ expected: 1, found: 0 }}, got {err:?}"
        );

        // version 2 as the first event is also a conflict.
        let err = store.append(&event("agg-1", "Created", 2)).unwrap_err();
        assert!(
            matches!(
                err,
                EventError::ConcurrencyConflict {
                    expected: 1,
                    found: 2
                }
            ),
            "expected ConcurrencyConflict {{ expected: 1, found: 2 }}, got {err:?}"
        );
    }

    #[test]
    fn append_monotonic_version_succeeds() {
        let store = InMemoryEventStore::new();
        store.append(&event("agg-1", "Created", 1)).unwrap();
        store.append(&event("agg-1", "Updated", 2)).unwrap();
        store.append(&event("agg-1", "Updated", 3)).unwrap();
        let events = store.get_events("agg-1").unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].metadata.event_type, "Created");
        assert_eq!(events[2].metadata.version, 3);
    }

    #[test]
    fn append_gap_returns_concurrency_conflict() {
        let store = InMemoryEventStore::new();
        store.append(&event("agg-1", "Created", 1)).unwrap();
        // Skipping version 2 -> 4 should be a conflict (expected 2, found 4).
        let err = store.append(&event("agg-1", "Updated", 4)).unwrap_err();
        assert!(
            matches!(
                err,
                EventError::ConcurrencyConflict {
                    expected: 2,
                    found: 4
                }
            ),
            "expected ConcurrencyConflict {{ expected: 2, found: 4 }}, got {err:?}"
        );
    }

    #[test]
    fn append_rejects_empty_aggregate_id() {
        let store = InMemoryEventStore::new();
        let err = store.append(&event("", "Created", 1)).unwrap_err();
        assert!(matches!(err, EventError::Store(_)), "got {err:?}");
    }

    #[test]
    fn append_rejects_empty_event_type() {
        let store = InMemoryEventStore::new();
        let err = store.append(&event("agg-1", "", 1)).unwrap_err();
        assert!(matches!(err, EventError::Store(_)), "got {err:?}");
    }

    #[test]
    fn append_rejects_oversized_identifier() {
        let store = InMemoryEventStore::new();
        let big = "x".repeat(MAX_ID_LENGTH + 1);
        let err = store.append(&event(&big, "Created", 1)).unwrap_err();
        assert!(matches!(err, EventError::Store(_)), "got {err:?}");
    }

    #[test]
    fn append_accepts_max_length_identifier() {
        let store = InMemoryEventStore::new();
        let exact = "x".repeat(MAX_ID_LENGTH);
        store
            .append(&event(&exact, "Created", 1))
            .expect("max-length id accepted");
    }
}
