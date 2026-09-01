//! Outbound event-bus port — publish-only bus from HexaKit phenotype-contracts.

use crate::error::Result;

/// Event bus port for publishing domain events.
///
/// Extracted from HexaKit `crates/phenotype-contracts` outbound ports.
pub trait EventBus: Send + Sync {
    /// Event message type published on the bus.
    type Message: Clone + Send + Sync;

    /// Publish a single event.
    fn publish(&self, event: Self::Message) -> Result<()>;

    /// Publish a batch of events atomically when supported by the adapter.
    fn publish_batch(&self, events: Vec<Self::Message>) -> Result<()>;
}
