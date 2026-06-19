//! Publish/subscribe event-bus ports from Eventra domain.

use crate::envelope::EventEnvelope;
use crate::error::Result;

/// Event handler port for pub/sub buses.
pub trait EventHandler: Send + Sync + EventHandlerClone {
    /// Handle a single event envelope.
    fn handle(&self, event: &EventEnvelope) -> Result<()>;

    /// Event type names this handler subscribes to.
    fn event_types(&self) -> Vec<String>;
}

/// Cloning helper for [`EventHandler`] trait objects.
pub trait EventHandlerClone {
    /// Clone this handler into a boxed trait object.
    fn clone_boxed(&self) -> Box<dyn EventHandler>;
}

/// Pub/sub event bus port — publish and subscribe with typed handlers.
pub trait PubSubBus: Send + Sync {
    /// Publish an event to all matching subscribers.
    fn publish(&self, event: &EventEnvelope) -> Result<()>;

    /// Register a handler for its declared event types.
    fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()>;
}
