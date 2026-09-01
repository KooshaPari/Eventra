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

    #[error("Serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_store() {
        let err = EventError::Store("connection lost".to_string());
        assert_eq!(err.to_string(), "Event store error: connection lost");
    }

    #[test]
    fn display_aggregate() {
        let err = EventError::Aggregate("invalid state".to_string());
        assert_eq!(err.to_string(), "Aggregate error: invalid state");
    }

    #[test]
    fn display_unknown_event_type() {
        let err = EventError::UnknownEventType("FooBar".to_string());
        assert_eq!(err.to_string(), "Event type not recognized: FooBar");
    }

    #[test]
    fn display_concurrency_conflict() {
        let err = EventError::ConcurrencyConflict {
            expected: 10,
            found: 12,
        };
        assert_eq!(
            err.to_string(),
            "Concurrency conflict: expected version 10, found 12"
        );
    }

    #[test]
    fn display_upcast() {
        let err = EventError::Upcast("schema mismatch".to_string());
        assert_eq!(err.to_string(), "Event upcasting error: schema mismatch");
    }

    #[test]
    fn display_serde_json() {
        let serde_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let event_err = EventError::SerdeJson(serde_err);
        assert!(event_err.to_string().starts_with("Serialization error:"));
    }

    #[test]
    fn from_serde_json_error() {
        let serde_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let event_err: EventError = serde_err.into();
        assert!(matches!(event_err, EventError::SerdeJson(_)));
        let msg = event_err.to_string();
        assert!(msg.starts_with("Serialization error:"));
    }
}
