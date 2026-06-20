//! Adapters Layer

pub mod event_store;
pub mod persistent_event_store;

pub use event_store::*;
pub use persistent_event_store::*;
