use crate::{EventBus, EventBusError, EventEnvelope, Subscription};
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

/// In-memory event bus for testing
pub struct InMemoryEventBus<T: Serialize + DeserializeOwned + Send + Sync + Clone + Debug + 'static>
{
    subscribers: Arc<DashMap<String, Vec<mpsc::UnboundedSender<EventEnvelope<T>>>>>,
    closed: Arc<RwLock<bool>>,
}

impl<T: Serialize + DeserializeOwned + Send + Sync + Clone + Debug + 'static> Default
    for InMemoryEventBus<T>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Serialize + DeserializeOwned + Send + Sync + Clone + Debug + 'static> InMemoryEventBus<T> {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(DashMap::new()),
            closed: Arc::new(RwLock::new(false)),
        }
    }
}

#[async_trait]
impl<T: Serialize + DeserializeOwned + Send + Sync + Clone + Debug + 'static> EventBus
    for InMemoryEventBus<T>
{
    type Event = T;

    async fn publish(&self, event: EventEnvelope<T>) -> Result<(), EventBusError> {
        let closed = *self.closed.read().await;
        if closed {
            return Err(EventBusError::Connection("Bus is closed".to_string()));
        }

        let subject = event.source.clone();

        // Send to exact subject subscribers
        if let Some(subs) = self.subscribers.get(&subject) {
            for sender in subs.iter() {
                let _ = sender.send(event.clone());
            }
        }

        // Send to wildcard subscribers
        for entry in self.subscribers.iter() {
            let pattern = entry.key();
            if pattern.ends_with('*') && subject.starts_with(&pattern[..pattern.len() - 1]) {
                for sender in entry.value().iter() {
                    let _ = sender.send(event.clone());
                }
            }
        }

        Ok(())
    }

    async fn subscribe<F>(&self, subject: &str, handler: F) -> Result<Subscription, EventBusError>
    where
        F: Fn(EventEnvelope<T>) -> Result<(), EventBusError> + Send + Sync + 'static,
    {
        let closed = *self.closed.read().await;
        if closed {
            return Err(EventBusError::Connection("Bus is closed".to_string()));
        }

        let (tx, mut rx) = mpsc::unbounded_channel();
        let sub_id = ulid::Ulid::new().to_string();

        self.subscribers
            .entry(subject.to_string())
            .or_default()
            .push(tx);

        // Spawn handler task
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if handler(event).is_err() {
                    break;
                }
            }
        });

        Ok(Subscription::new(sub_id, subject.to_string()))
    }

    async fn request<R: Serialize + DeserializeOwned + Send + Sync + Clone + Debug + 'static>(
        &self,
        _subject: &str,
        _payload: R,
        _timeout_ms: u64,
    ) -> Result<EventEnvelope<R>, EventBusError> {
        // In-memory request-response requires a dedicated mechanism
        // For now, return an error indicating this is not implemented for in-memory bus
        Err(EventBusError::Connection(
            "Request-response not supported on in-memory bus".to_string(),
        ))
    }

    async fn close(&self) -> Result<(), EventBusError> {
        let mut closed = self.closed.write().await;
        *closed = true;
        self.subscribers.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestEvent {
        message: String,
    }

    #[tokio::test]
    async fn fr_memory_event_bus_001_publish_subscribe() {
        let bus = InMemoryEventBus::<TestEvent>::new();
        let received = Arc::new(RwLock::new(Vec::new()));
        let received_clone = received.clone();

        bus.subscribe("test.subject", move |envelope| {
            let rt = tokio::runtime::Handle::current();
            let received = received_clone.clone();
            rt.spawn(async move {
                received.write().await.push(envelope.payload);
            });
            Ok(())
        })
        .await
        .unwrap();

        let event = TestEvent {
            message: "hello".to_string(),
        };
        bus.publish(EventEnvelope::new("test.subject", event))
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let messages = received.read().await;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message, "hello");
    }

    #[tokio::test]
    async fn fr_memory_event_bus_002_wildcard_subscription() {
        let bus = InMemoryEventBus::<TestEvent>::new();
        let received = Arc::new(RwLock::new(Vec::new()));
        let received_clone = received.clone();

        bus.subscribe("test.*", move |envelope| {
            let rt = tokio::runtime::Handle::current();
            let received = received_clone.clone();
            rt.spawn(async move {
                received.write().await.push(envelope.payload);
            });
            Ok(())
        })
        .await
        .unwrap();

        let event1 = TestEvent {
            message: "event1".to_string(),
        };
        let event2 = TestEvent {
            message: "event2".to_string(),
        };

        bus.publish(EventEnvelope::new("test.foo", event1))
            .await
            .unwrap();
        bus.publish(EventEnvelope::new("test.bar", event2))
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let messages = received.read().await;
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn fr_memory_event_bus_003_close_bus() {
        let bus = InMemoryEventBus::<TestEvent>::new();
        bus.close().await.unwrap();

        let result = bus
            .publish(EventEnvelope::new(
                "test",
                TestEvent {
                    message: "test".to_string(),
                },
            ))
            .await;

        assert!(matches!(result, Err(EventBusError::Connection(_))));
    }
}
