//! Aggregate - Domain Entity

use std::collections::VecDeque;

use super::{Command, Event, error::EventError};

/// Aggregate root trait
pub trait Aggregate: Send {
    fn id(&self) -> &str;
    fn version(&self) -> u32;
    fn uncommitted_events(&self) -> Vec<Event>;
    fn mark_events_committed(&mut self);
    fn apply(&mut self, event: &Event) -> Result<(), EventError>;
<<<<<<< HEAD
    /// Rehydrate an aggregate from its historical event stream.
=======

>>>>>>> 65d9cd6 (feat(eventra): fix aggregate replay and event bus cloning)
    fn load_from_events(&mut self, events: &[Event]) -> Result<(), EventError> {
        for event in events {
            self.apply(event)?;
        }
        Ok(())
    }
<<<<<<< HEAD
    /// Execute a command, producing the events that result from it.
    fn execute(&mut self, command: super::Command) -> Result<Vec<Event>, EventError>;
=======

    fn execute(&mut self, _command: Command) -> Result<Vec<Event>, EventError>;
>>>>>>> 65d9cd6 (feat(eventra): fix aggregate replay and event bus cloning)
}

/// Base aggregate implementation
pub struct BaseAggregate {
    id: String,
    version: u32,
    uncommitted: VecDeque<Event>,
}

impl BaseAggregate {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: 0,
            uncommitted: VecDeque::new(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn uncommitted_events(&self) -> Vec<Event> {
        self.uncommitted.iter().cloned().collect()
    }

    pub fn mark_events_committed(&mut self) {
        self.uncommitted.clear();
    }

    pub fn add_event(&mut self, event: Event) {
        self.version += 1;
        self.uncommitted.push_back(event);
    }

    pub fn apply(&mut self, event: &Event) -> Result<(), EventError> {
        self.version += 1;
        self.uncommitted.push_back(event.clone());
        Ok(())
    }

    pub fn load_from_events(&mut self, events: &[Event]) -> Result<(), EventError> {
        for event in events {
            self.apply(event)?;
        }
        Ok(())
    }
}

impl Aggregate for BaseAggregate {
    fn id(&self) -> &str {
        BaseAggregate::id(self)
    }

    fn version(&self) -> u32 {
        BaseAggregate::version(self)
    }

    fn uncommitted_events(&self) -> Vec<Event> {
        BaseAggregate::uncommitted_events(self)
    }

    fn mark_events_committed(&mut self) {
        BaseAggregate::mark_events_committed(self);
    }

    fn apply(&mut self, _event: &Event) -> Result<(), EventError> {
        self.version += 1;
        Ok(())
    }

    fn execute(&mut self, _command: Command) -> Result<Vec<Event>, EventError> {
        Err(EventError::Aggregate(
            "BaseAggregate cannot execute commands directly".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{BaseAggregate, Event};

    #[test]
    fn load_from_events_replays_events_and_advances_version() {
        let mut aggregate = BaseAggregate::new("agg-1");
        let events = vec![
            Event::new("agg-1", "aggregate", "created", 1, json!({})),
            Event::new("agg-1", "aggregate", "updated", 2, json!({})),
        ];

        aggregate.load_from_events(&events).expect("replay succeeds");

        assert_eq!(aggregate.version(), 2);
    }
}
