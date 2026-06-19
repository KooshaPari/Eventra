//! # Events Module
//!
//! Domain events for the event sourcing system.

use serde::{Deserialize, Serialize};

/// Marker trait for domain events.
///
/// Domain events should implement this trait to be used with EventEnvelope.
pub trait DomainEvent: Serialize + for<'de> Deserialize<'de> + Send + Sync {
    /// The event type name (used for discrimination).
    fn event_type(&self) -> &'static str;
}


