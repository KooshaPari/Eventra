//! # Domain Layer
//!
//! Pure domain logic for event sourcing with **ZERO external dependencies**.
//!
//! This layer contains:
//! - [`entities`] - EventEnvelope, aggregate roots
//! - [`services`] - Domain services for hash computation, verification
//! - [`events`] - Domain events
//! - [`ports`] - Port interfaces for storage adapters
//!
//! ## Dependency Rule
//!
//! ```text
//! Domain ──NO DEPENDENCIES──► Application/Adapters
//! ```

pub mod entities;
pub mod services;
pub mod events;
pub mod ports;

// Re-export commonly used types
pub use entities::*;
pub use services::*;
pub use events::*;
pub use ports::*;
