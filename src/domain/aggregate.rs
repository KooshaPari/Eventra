//! Aggregate - Domain Entity

use std::collections::VecDeque;
use serde::{Deserialize, Serialize};

use super::{Event, error::EventError};

/// Aggregate root trait
pub trait Aggregate: Send {
    fn id(&self) -> &str;
    fn version(&self) -> u32;
    fn uncommitted_events(&self) -> Vec<Event>;
    fn mark_events_committed(&mut self);
    fn apply(&mut self, event: &Event) -> Result<(), EventError>;
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

    pub fn load_from_events(&mut self, events: &[Event]) -> Result<(), EventError> {
        for event in events {
            self.apply(event)?;
        }
        Ok(())
    }
}
