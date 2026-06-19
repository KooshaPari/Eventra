//! # Outbound Adapters
//!
//! Implementations of outbound ports for various storage backends.

pub mod persistence;
pub mod inmemory;

pub use persistence::*;
pub use inmemory::*;
