//! Command - Value Object

use serde::{Deserialize, Serialize};

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

/// Command handler trait
pub trait CommandHandler<C: Send + 'static>: Send + Sync {
    fn handle(&self, command: C) -> Result<(), super::error::EventError>;
}
