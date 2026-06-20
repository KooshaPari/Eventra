//! Command - Value Object

use serde::{Deserialize, Serialize};

use super::error::EventError;

/// Command - represents intent to change state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub command_type: String,
    pub aggregate_id: String,
    pub payload: serde_json::Value,
    pub metadata: CommandMetadata,
}

impl Command {
    pub fn new(
        command_type: impl Into<String>,
        aggregate_id: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            command_type: command_type.into(),
            aggregate_id: aggregate_id.into(),
            payload,
            metadata: CommandMetadata::default(),
        }
    }
}

/// Command metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub user_id: Option<String>,
    pub trace_id: Option<String>,
}

impl Default for CommandMetadata {
    fn default() -> Self {
        Self {
            user_id: None,
            trace_id: None,
        }
    }
}

impl super::Validate for Command {
    fn validate(&self) -> Result<(), EventError> {
        if self.command_type.trim().is_empty() {
            return Err(EventError::Validation("command_type must not be empty".into()));
        }
        if self.aggregate_id.trim().is_empty() {
            return Err(EventError::Validation("aggregate_id must not be empty".into()));
        }
        let size = serde_json::to_vec(&self.payload).map(|v| v.len()).unwrap_or(0);
        if size > super::event::MAX_PAYLOAD_BYTES {
            return Err(EventError::Validation(format!(
                "payload exceeds maximum size of {} bytes",
                super::event::MAX_PAYLOAD_BYTES
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Validate;
    use serde_json::json;

    #[test]
    fn valid_command_passes_validation() {
        let command = Command::new("CreateOrder", "order-1", json!({ "item": "book" }));
        assert!(command.validate().is_ok());
    }

    #[test]
    fn empty_command_type_is_rejected() {
        let command = Command::new("", "order-1", json!({}));
        assert!(matches!(command.validate(), Err(EventError::Validation(_))));
    }

    #[test]
    fn empty_aggregate_id_is_rejected() {
        let command = Command::new("CreateOrder", "   ", json!({}));
        assert!(matches!(command.validate(), Err(EventError::Validation(_))));
    }

    #[test]
    fn oversized_payload_is_rejected() {
        let huge = "x".repeat(crate::domain::event::MAX_PAYLOAD_BYTES + 1);
        let command = Command::new("CreateOrder", "order-1", json!({ "data": huge }));
        assert!(matches!(command.validate(), Err(EventError::Validation(_))));
    }
}
