//! # Ports
//!
//! Port interfaces define the boundaries of the domain.
//!
//! ## Port Types
//!
//! - **Inbound Ports (Primary/Driving)**: Define what the application can do
//! - **Outbound Ports (Secondary/Driven)**: Define what infrastructure must provide

pub mod inbound;
pub mod outbound;

// Re-export port traits for convenience
pub use inbound::*;
pub use outbound::*;
