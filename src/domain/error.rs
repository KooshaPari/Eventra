//! Domain Errors

use std::fmt;

#[derive(Debug)]
pub enum EventError {
    Store(String),
    Aggregate(String),
    UnknownEventType(String),
    ConcurrencyConflict { expected: u32, found: u32 },
    Upcast(String),
    SerdeJson(serde_json::Error),
}

impl fmt::Display for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventError::Store(msg) => write!(f, "Event store error: {msg}"),
            EventError::Aggregate(msg) => write!(f, "Aggregate error: {msg}"),
            EventError::UnknownEventType(ty) => write!(f, "Event type not recognized: {ty}"),
            EventError::ConcurrencyConflict { expected, found } => {
                write!(
                    f,
                    "Concurrency conflict: expected version {expected}, found {found}"
                )
            }
            EventError::Upcast(msg) => write!(f, "Event upcasting error: {msg}"),
            EventError::SerdeJson(err) => write!(f, "Serialization error: {err}"),
        }
    }
}

impl std::error::Error for EventError {}

impl From<serde_json::Error> for EventError {
    fn from(err: serde_json::Error) -> Self {
        EventError::SerdeJson(err)
    }
}

pub type EventResult<T> = Result<T, EventError>;

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
        assert!(event_err.to_string().contains("Serialization error:"));
    }

    #[test]
    fn from_serde_json_error() {
        let serde_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let event_err: EventError = serde_err.into();
        assert!(matches!(event_err, EventError::SerdeJson(_)));
        let msg = event_err.to_string();
        assert!(msg.contains("expected") || msg.contains("invalid"));
    }
}
