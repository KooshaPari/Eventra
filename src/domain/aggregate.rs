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

    /// Rebuild the aggregate state by replaying a list of committed events.
    fn load_from_events(&mut self, events: &[Event]) -> Result<(), EventError> {
        for event in events {
            self.apply(event)?;
        }
        Ok(())
    }

    /// Execute a command against this aggregate, returning the events it
    /// produced. The default implementation is a no-op that returns no events;
    /// concrete aggregates should override it to apply command-specific logic.
    fn execute(&mut self, _command: Command) -> Result<Vec<Event>, EventError> {
        Ok(Vec::new())
    }
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

    /// Re-apply an event without producing uncommitted events. Used when
    /// rebuilding state from the event store: the event is assumed already
    /// committed, so only the version is advanced.
    pub fn apply(&mut self, _event: &Event) -> Result<(), EventError> {
        self.version += 1;
        Ok(())
    }

    pub fn load_from_events(&mut self, events: &[Event]) -> Result<(), EventError> {
        for event in events {
            self.apply(event)?;
        }
        Ok(())
    }
}
