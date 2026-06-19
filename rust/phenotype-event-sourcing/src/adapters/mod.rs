//! # Adapters Layer
//!
//! Infrastructure adapters implement outbound ports.

pub mod outbound;

// Re-export for convenience
pub use outbound::*;
