//! Domain Errors

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EventError {
    #[error("Event store error: {0}")]
    Store(String),

    #[error("Aggregate error: {0}")]
    Aggregate(String),

    #[error("Event type not recognized: {0}")]
    UnknownEventType(String),

    #[error("Concurrency conflict: expected version {expected}, found {found}")]
    ConcurrencyConflict { expected: u32, found: u32 },

    #[error("Event upcasting error: {0}")]
    Upcast(String),
}

pub type EventResult<T> = Result<T, EventError>;
