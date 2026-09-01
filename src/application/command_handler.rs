//! Command Handler

use crate::domain::{Aggregate, Command, EventError, EventStore};

/// Command handler service
pub struct CommandHandlerService<A: Aggregate> {
    event_store: Box<dyn EventStore>,
    aggregate_factory: Box<dyn AggregateFactory<A>>,
}

impl<A: Aggregate> CommandHandlerService<A> {
    pub fn new(
        event_store: Box<dyn EventStore>,
        aggregate_factory: Box<dyn AggregateFactory<A>>,
    ) -> Self {
        Self {
            event_store,
            aggregate_factory,
        }
    }

    pub fn handle(&self, command: Command) -> Result<A, EventError> {
        // Load aggregate
        let events = self.event_store.get_events(&command.aggregate_id)?;
        let mut aggregate = self.aggregate_factory.create(&command.aggregate_id)?;
        aggregate.load_from_events(&events)?;

        // Execute command (returns events)
        let new_events = aggregate.execute(command)?;

        // Append events
        for event in &new_events {
            self.event_store.append(event)?;
        }

        // Mark committed
        aggregate.mark_events_committed();

        Ok(aggregate)
    }
}

/// Aggregate factory trait
pub trait AggregateFactory<A: Aggregate>: Send + Sync {
    fn create(&self, id: &str) -> Result<A, EventError>;
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use serde_json::json;

    use crate::domain::{Aggregate, Command, Event, EventError, EventMetadata, EventStore};

    use super::{AggregateFactory, CommandHandlerService};

    #[derive(Default)]
    struct MemoryStore {
        events: Arc<Mutex<Vec<Event>>>,
        appended: Arc<Mutex<Vec<Event>>>,
    }

    impl EventStore for MemoryStore {
        fn append(&self, event: &Event) -> Result<(), EventError> {
            self.appended.lock().expect("lock").push(event.clone());
            Ok(())
        }

        fn get_events(&self, _aggregate_id: &str) -> Result<Vec<Event>, EventError> {
            Ok(self.events.lock().expect("lock").clone())
        }

        fn get_events_since(
            &self,
            _since: chrono::DateTime<chrono::Utc>,
        ) -> Result<Vec<Event>, EventError> {
            Ok(Vec::new())
        }

        fn get_all_events(&self) -> Result<Vec<Event>, EventError> {
            Ok(self.events.lock().expect("lock").clone())
        }
    }

    #[derive(Default)]
    struct TestAggregate {
        id: String,
        version: u32,
        uncommitted: Vec<Event>,
    }

    impl Aggregate for TestAggregate {
        fn id(&self) -> &str {
            &self.id
        }

        fn version(&self) -> u32 {
            self.version
        }

        fn uncommitted_events(&self) -> Vec<Event> {
            self.uncommitted.clone()
        }

        fn mark_events_committed(&mut self) {
            self.uncommitted.clear();
        }

        fn apply(&mut self, _event: &Event) -> Result<(), EventError> {
            self.version += 1;
            Ok(())
        }

        fn execute(&mut self, command: Command) -> Result<Vec<Event>, EventError> {
            let event = Event {
                metadata: EventMetadata::new(
                    self.id.clone(),
                    "test-aggregate",
                    command.command_type,
                    self.version + 1,
                ),
                payload: command.payload,
            };
            self.uncommitted.push(event.clone());
            self.version += 1;
            Ok(vec![event])
        }
    }

    struct TestAggregateFactory;

    impl AggregateFactory<TestAggregate> for TestAggregateFactory {
        fn create(&self, id: &str) -> Result<TestAggregate, EventError> {
            Ok(TestAggregate {
                id: id.to_string(),
                ..Default::default()
            })
        }
    }

    #[test]
    fn handle_rehydrates_executes_and_persists_new_events() {
        let store = MemoryStore::default();
        store.events.lock().expect("lock").push(Event::new(
            "agg-1",
            "test-aggregate",
            "created",
            1,
            json!({ "seed": true }),
        ));

        let service = CommandHandlerService::new(Box::new(store), Box::new(TestAggregateFactory));
        let command = Command::new("updated", "agg-1", json!({ "value": 1 }));

        let aggregate = service.handle(command).expect("command handles");

        assert_eq!(aggregate.version(), 2);
        assert_eq!(aggregate.uncommitted_events().len(), 0);
    }
}
