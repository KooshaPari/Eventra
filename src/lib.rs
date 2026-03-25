//! # eventkit - Event-Driven Architecture Framework
//!
//! CQRS and Event Sourcing with hexagonal architecture.

pub mod domain;
pub mod application;
pub mod adapters;
pub mod infrastructure;

pub use domain::*;
pub use application::*;
