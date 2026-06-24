//! Projection System

use parking_lot::RwLock;
use std::collections::HashMap;

use crate::domain::{Event, EventError, EventStore};

/// Projection definition
pub trait Projection: Send + Sync {
    fn name(&self) -> &str;
    fn handles(&self) -> &[String];
    fn apply(&mut self, event: &Event) -> Result<(), EventError>;
}

/// Projection state
#[derive(Debug, Clone)]
pub struct ProjectionState {
    pub name: String,
    pub position: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Projection runner
pub struct ProjectionRunner {
    projections: RwLock<HashMap<String, Box<dyn Projection>>>,
    event_store: Box<dyn EventStore>,
    state: RwLock<HashMap<String, ProjectionState>>,
}

impl ProjectionRunner {
    pub fn new(event_store: Box<dyn EventStore>) -> Self {
        Self {
            projections: RwLock::new(HashMap::new()),
            event_store,
            state: RwLock::new(HashMap::new()),
        }
    }

    pub fn register<P: Projection + 'static>(&self, projection: P) {
        let name = projection.name().to_string();
        drop(
            self.projections
                .write()
                .insert(name.clone(), Box::new(projection)),
        );
        drop(self.state.write().insert(
            name.clone(),
            ProjectionState {
                name,
                position: 0,
                last_updated: chrono::Utc::now(),
            },
        ));
    }

    /// Replay events for every registered projection starting from its saved
    /// position (or from offset 0 for first-time runs). Each projection tracks
    /// its own position via [`ProjectionState`].
    pub fn run(&self) -> Result<(), EventError> {
        let events = self.event_store.get_all_events()?;
        let mut projections = self.projections.write();
        let mut state = self.state.write();

        for (idx, event) in events.iter().enumerate() {
            for (_, projection) in projections.iter_mut() {
                if projection.handles().contains(&event.metadata.event_type) {
                    if let Some(state) = state.get_mut(projection.name()) {
                        let position = idx as u64;
                        if position < state.position {
                            continue;
                        }
                        projection.apply(event)?;
                        state.position = position + 1;
                        state.last_updated = chrono::Utc::now();
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_state(&self, name: &str) -> Option<ProjectionState> {
        self.state.read().get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::event_store::InMemoryEventStore;
    use crate::domain::Event;
    use std::sync::Arc;

    /// Test projection that counts how many `Counted` events it has seen.
    struct CountingProjection {
        name: String,
        count: Arc<parking_lot::Mutex<u32>>,
        handled: Vec<String>,
    }

    impl CountingProjection {
        fn new(name: &str, count: Arc<parking_lot::Mutex<u32>>) -> Self {
            Self {
                name: name.to_string(),
                count,
                handled: vec!["Counted".to_string()],
            }
        }
    }

    impl Projection for CountingProjection {
        fn name(&self) -> &str {
            &self.name
        }
        fn handles(&self) -> &[String] {
            &self.handled
        }
        fn apply(&mut self, _event: &Event) -> Result<(), EventError> {
            *self.count.lock() += 1;
            Ok(())
        }
    }

    fn make_event(aggregate_id: &str, event_type: &str, version: u32) -> Event {
        Event::new(
            aggregate_id,
            "TestAggregate",
            event_type,
            version,
            serde_json::json!({}),
        )
    }

    #[test]
    fn run_with_zero_events_is_a_noop() {
        let store = Box::new(InMemoryEventStore::new());
        let runner = ProjectionRunner::new(store);
        let count = Arc::new(parking_lot::Mutex::new(0));
        runner.register(CountingProjection::new("counter", count.clone()));

        // Run on an empty event store — must succeed and not increment the counter.
        runner.run().expect("run with zero events should succeed");

        assert_eq!(
            *count.lock(),
            0,
            "no events were appended, so count must stay 0"
        );
        let state = runner
            .get_state("counter")
            .expect("projection state should exist");
        assert_eq!(state.position, 0, "position must remain 0 with no events");
    }

    #[test]
    fn run_with_three_events_applies_each_in_order() {
        let store = Box::new(InMemoryEventStore::new());
        // Append three events: 1 irrelevant + 2 of type "Counted".
        store.append(&make_event("agg-1", "Other", 1)).unwrap();
        store.append(&make_event("agg-1", "Counted", 2)).unwrap();
        store.append(&make_event("agg-1", "Counted", 3)).unwrap();

        let runner = ProjectionRunner::new(store);
        let count = Arc::new(parking_lot::Mutex::new(0));
        runner.register(CountingProjection::new("counter", count.clone()));

        runner.run().expect("run with three events should succeed");

        assert_eq!(
            *count.lock(),
            2,
            "exactly the two Counted events should have been applied"
        );
        let state = runner
            .get_state("counter")
            .expect("projection state should exist");
        assert_eq!(
            state.position, 3,
            "position must advance to last event index + 1"
        );
    }
}
