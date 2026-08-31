//! Event Bus

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::domain::{Event, EventBus, EventError, EventHandler};

/// Simple in-memory event bus
pub struct InMemoryEventBus {
    subscribers: RwLock<HashMap<String, Vec<Arc<Box<dyn EventHandler>>>>>,
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
        let mut subscribers = self.subscribers.write();
        // Share a single handler across every subscribed event type by cloning
        // the Arc, avoiding the need to clone the trait object itself.
        let shared = Arc::new(handler);

        for event_type in event_types {
            let entry = subscribers.entry(event_type).or_insert_with(Vec::new);
            entry.push(shared.clone());
        }

        Ok(())
    }
}
