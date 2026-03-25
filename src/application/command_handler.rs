//! Command Handler

use async_trait::async_trait;

use crate::domain::{Aggregate, Command, EventError, EventStore};

/// Command handler service
pub struct CommandHandlerService<S: EventStore, A: Aggregate> {
    event_store: Box<dyn EventStore>,
    aggregate_factory: Box<dyn AggregateFactory<A>>,
}

impl<S: EventStore, A: Aggregate> CommandHandlerService<S, A> {
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
