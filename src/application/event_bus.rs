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

        for event_type in event_types {
            let entry = subscribers.entry(event_type).or_insert_with(Vec::new);
            entry.push(Arc::new(handler.clone_boxed()));
        }

        Ok(())
    }
}

impl Clone for Box<dyn EventHandler> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

pub trait EventHandlerClone {
    fn clone_boxed(&self) -> Box<dyn EventHandler>;
}

impl<T: EventHandler + Clone> EventHandlerClone for T {
    fn clone_boxed(&self) -> Box<dyn EventHandler> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn EventHandler> {
    fn clone(&self) -> Box<dyn EventHandler> {
        self.clone_boxed()
    }
}
