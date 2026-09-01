//! # Persistence Adapters
//!
//! Database adapter implementations.
//!
//! ## Feature Flags
//!
//! - `persistence-sqlx` - PostgreSQL adapter using sqlx
//! - `persistence-sqlite` - SQLite adapter using rusqlite

#[cfg(feature = "persistence-sqlx")]
pub mod sqlx;

#[cfg(feature = "persistence-sqlite")]
pub mod sqlite;
