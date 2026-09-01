//! Core contract trait for domain events and messages.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Core contract trait for domain events and messages.
///
/// Defines the basic contract interface used across the phenotype ecosystem for
/// event sourcing, messaging, and domain event handling.
pub trait Contract: Send + Sync {
    /// Returns the type of this contract.
    fn contract_type(&self) -> &'static str;

    /// Returns the timestamp when this contract was created.
    fn timestamp(&self) -> DateTime<Utc>;

    /// Returns the correlation ID for tracking related events.
    fn correlation_id(&self) -> Uuid;

    /// Returns a reference to this object as `Any` for downcasting.
    fn as_any(&self) -> &dyn std::any::Any;
}
