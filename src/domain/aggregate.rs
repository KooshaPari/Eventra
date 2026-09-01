//! Aggregate - Domain Entity

use std::collections::VecDeque;

use super::{error::EventError, Command, Event};

/// Aggregate root trait
pub trait Aggregate: Send {
    fn id(&self) -> &str;
    fn version(&self) -> u32;
    fn uncommitted_events(&self) -> Vec<Event>;
    fn mark_events_committed(&mut self);
    fn apply(&mut self, event: &Event) -> Result<(), EventError>;
    /// Rehydrate an aggregate from its historical event stream.
    fn load_from_events(&mut self, events: &[Event]) -> Result<(), EventError> {
        for event in events {
            self.apply(event)?;
        }
        Ok(())
    }
    /// Execute a command, producing the events that result from it.
    fn execute(&mut self, command: Command) -> Result<Vec<Event>, EventError>;
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
        &self.id
    }

    fn version(&self) -> u32 {
        self.version
    }

    fn uncommitted_events(&self) -> Vec<Event> {
        self.uncommitted.iter().cloned().collect()
    }

    fn mark_events_committed(&mut self) {
        self.uncommitted.clear();
    }

    fn apply(&mut self, event: &Event) -> Result<(), EventError> {
        self.version += 1;
        self.uncommitted.push_back(event.clone());
        Ok(())
    }

    fn execute(&mut self, _command: Command) -> Result<Vec<Event>, EventError> {
        Err(EventError::Aggregate("execute not implemented".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_aggregate_has_zero_version_and_no_events() {
        let agg = BaseAggregate::new("agg-1");
        assert_eq!(agg.id(), "agg-1");
        assert_eq!(agg.version(), 0);
        assert!(agg.uncommitted_events().is_empty());
    }

    #[test]
    fn add_event_bumps_version_and_records_event() {
        let mut agg = BaseAggregate::new("agg-2");
        let event = Event::new(
            "agg-2",
            "TestAggregate",
            "Created",
            1,
            serde_json::json!({}),
        );
        agg.add_event(event.clone());

        assert_eq!(agg.version(), 1);
        let events = agg.uncommitted_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].metadata.aggregate_id, "agg-2");
        assert_eq!(events[0].metadata.event_type, "Created");
    }

    #[test]
    fn mark_events_committed_clears_pending_events_but_keeps_version() {
        let mut agg = BaseAggregate::new("agg-3");
        agg.add_event(Event::new(
            "agg-3",
            "TestAggregate",
            "Created",
            1,
            serde_json::json!({}),
        ));
        assert_eq!(agg.version(), 1);
        assert_eq!(agg.uncommitted_events().len(), 1);

        agg.mark_events_committed();

        assert_eq!(agg.version(), 1, "version should not reset on commit");
        assert!(agg.uncommitted_events().is_empty());
    }

    #[test]
    fn aggregate_trait_delegates_to_inherent_methods() {
        // Verify that the Aggregate trait impl works through a trait object.
        let mut agg: Box<dyn Aggregate> = Box::new(BaseAggregate::new("agg-4"));
        assert_eq!(agg.id(), "agg-4");
        assert_eq!(agg.version(), 0);

        let event = Event::new(
            "agg-4",
            "TestAggregate",
            "Touched",
            1,
            serde_json::json!({}),
        );
        agg.apply(&event).expect("apply should succeed");
        assert_eq!(agg.version(), 1);
        assert_eq!(agg.uncommitted_events().len(), 1);

        agg.mark_events_committed();
        assert!(agg.uncommitted_events().is_empty());
    }

    #[test]
    fn load_from_events_replays_events_and_advances_version() {
        let mut aggregate = BaseAggregate::new("agg-1");
        let events = vec![
            Event::new("agg-1", "aggregate", "created", 1, serde_json::json!({})),
            Event::new("agg-1", "aggregate", "updated", 2, serde_json::json!({})),
        ];

        aggregate
            .load_from_events(&events)
            .expect("replay succeeds");

        assert_eq!(aggregate.version(), 2);
    }
}
