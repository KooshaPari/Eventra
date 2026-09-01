//! Domain event contract trait.

use crate::contract::Contract;

/// Event trait for domain events.
///
/// Extends [`Contract`] with event-sourcing properties like aggregate ID and
/// sequence number.
pub trait Event: Contract {
    /// Returns the aggregate ID this event belongs to.
    fn aggregate_id(&self) -> &str;

    /// Returns the sequence number of this event in the aggregate's stream.
    fn sequence(&self) -> u64;
}
