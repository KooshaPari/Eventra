//! Event Bus

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use tracing::{debug, info, instrument};

use crate::domain::{Event, EventBus, EventError, EventHandler};

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
    #[instrument(
        name = "event_bus.publish",
        level = "info",
        skip(self, event),
        fields(
            event_id = %event.metadata.event_id,
            event_type = %event.metadata.event_type,
            aggregate_id = %event.metadata.aggregate_id,
            aggregate_type = %event.metadata.aggregate_type,
            version = event.metadata.version,
        )
    )]
    fn publish(&self, event: &Event) -> Result<(), EventError> {
        let event_type = &event.metadata.event_type;
        let subscribers = self.subscribers.read();

        match subscribers.get(event_type) {
            Some(handlers) if !handlers.is_empty() => {
                let handler_count = handlers.len();
                info!(
                    target: "eventkit::event_bus",
                    event_type = %event_type,
                    handlers = handler_count,
                    "publishing event to {handler_count} handler(s)"
                );
                for handler in handlers {
                    if let Err(err) = handler.handle(event) {
                        // Log at error level but do not abort the publish — one
                        // failing handler must not block the rest of the chain.
                        tracing::error!(
                            target: "eventkit::event_bus",
                            event_type = %event_type,
                            error = %err,
                            "event handler returned error"
                        );
                    }
                }
                Ok(())
            }
            Some(_) | None => {
                debug!(
                    target: "eventkit::event_bus",
                    event_type = %event_type,
                    "no subscribers registered for event_type"
                );
                Ok(())
            }
        }
    }

    #[instrument(
        name = "event_bus.subscribe",
        level = "info",
        skip(self, handler),
        fields(event_types = ?handler.event_types())
    )]
    fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<(), EventError> {
        let event_types = handler.event_types();
        let handler = Arc::from(handler);
        let mut subscribers = self.subscribers.write();

        for event_type in &event_types {
            let entry = subscribers
                .entry(event_type.clone())
                .or_insert_with(Vec::new);
            entry.push(Arc::clone(&handler));
        }

        info!(
            target: "eventkit::event_bus",
            event_types = ?event_types,
            "subscribed handler to {event_count} event type(s)",
            event_count = event_types.len()
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::json;

    use crate::domain::{Event, EventBus, EventHandler, EventHandlerClone};

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

    impl EventHandlerClone for CountingHandler {
        fn clone_boxed(&self) -> Box<dyn EventHandler> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn subscribe_clones_handler_per_event_type_without_requiring_box_clone() {
        let bus = InMemoryEventBus::new();
        let handler = CountingHandler::default();
        let hits = handler.hits.clone();

        bus.subscribe(Box::new(handler))
            .expect("subscribe succeeds");

        let event = Event::new("agg-1", "aggregate", "event.created", 1, json!({}));
        bus.publish(&event).expect("publish succeeds");

        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
