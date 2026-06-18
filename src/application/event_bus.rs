//! Event Bus

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::domain::{Event, EventBus, EventError, EventHandler, EventHandlerClone};

/// Simple in-memory event bus
pub struct InMemoryEventBus {
    subscribers: RwLock<HashMap<String, Vec<Arc<dyn EventHandler>>>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self {
            subscribers: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus for InMemoryEventBus {
    fn publish(&self, event: &Event) -> Result<(), EventError> {
        let event_type = &event.metadata.event_type;
        let subscribers = self.subscribers.read();

        if let Some(handlers) = subscribers.get(event_type) {
            for handler in handlers {
                handler.handle(event)?;
            }
        }

        Ok(())
    }

    fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<(), EventError> {
        let event_types = handler.event_types();
        let handler = Arc::from(handler);
        let mut subscribers = self.subscribers.write();

        for event_type in event_types {
            let entry = subscribers.entry(event_type).or_insert_with(Vec::new);
            entry.push(Arc::clone(&handler));
        }

        Ok(())
    }
}

<<<<<<< HEAD
impl Clone for Box<dyn EventHandler> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

impl<T: EventHandler + Clone + 'static> EventHandlerClone for T {
    fn clone_boxed(&self) -> Box<dyn EventHandler> {
        Box::new(self.clone())
=======
#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::json;

    use crate::domain::{Event, EventBus, EventHandler};

    use super::InMemoryEventBus;

    #[derive(Clone, Default)]
    struct CountingHandler {
        hits: std::sync::Arc<AtomicUsize>,
    }

    impl EventHandler for CountingHandler {
        fn handle(&self, _event: &Event) -> Result<(), crate::domain::EventError> {
            self.hits.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn event_types(&self) -> Vec<String> {
            vec!["event.created".to_string(), "event.updated".to_string()]
        }
    }

    #[test]
    fn subscribe_clones_handler_per_event_type_without_requiring_box_clone() {
        let bus = InMemoryEventBus::new();
        let handler = CountingHandler::default();
        let hits = handler.hits.clone();

        bus.subscribe(Box::new(handler)).expect("subscribe succeeds");

        let event = Event::new("agg-1", "aggregate", "event.created", 1, json!({}));
        bus.publish(&event).expect("publish succeeds");

        assert_eq!(hits.load(Ordering::SeqCst), 1);
>>>>>>> 65d9cd6 (feat(eventra): fix aggregate replay and event bus cloning)
    }
}
