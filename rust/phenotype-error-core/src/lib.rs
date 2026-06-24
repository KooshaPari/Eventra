//! `phenotype-error-core`
//!
//! Shared error types used across the Phenotype ecosystem.
//!
//! This crate was originally consumed from a git dependency on
//! `https://github.com/KooshaPari/phenotype-types` which no longer resolves.
//! To keep the `Eventra` workspace hermetic and CI reproducible, the small
//! surface area required by downstream crates is vendored here.
//!
//! Only the error types actually consumed by Eventra are provided:
//! [`RepositoryError`] and [`StorageError`].

use thiserror::Error;

/// Errors raised by repository (aggregate / projection) operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    /// Underlying storage backend reported a failure.
    #[error("repository storage error: {0}")]
    Storage(String),

    /// Aggregate or projection was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Optimistic concurrency conflict (e.g. unexpected aggregate version).
    #[error("concurrency conflict: {0}")]
    ConcurrencyConflict(String),

    /// Operation rejected as invalid before reaching storage.
    #[error("invalid operation: {0}")]
    Invalid(String),
}

/// Errors raised by storage adapters (in-memory today; Postgres/Kafka later).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// Connection to the backend could not be established or was lost.
    #[error("storage connection error: {0}")]
    Connection(String),

    /// Read or write operation failed at the backend.
    #[error("storage I/O error: {0}")]
    Io(String),

    /// Serialization or deserialization of a payload failed.
    #[error("storage serialization error: {0}")]
    Serialization(String),

    /// Operation exceeded the configured timeout.
    #[error("storage timeout")]
    Timeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_error_display_is_informative() {
        let err = RepositoryError::NotFound("agg-1".to_string());
        assert_eq!(err.to_string(), "not found: agg-1");
    }

    #[test]
    fn storage_error_display_is_informative() {
        let err = StorageError::Connection("refused".to_string());
        assert_eq!(err.to_string(), "storage connection error: refused");
    }

    #[test]
    fn errors_implement_std_error() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<RepositoryError>();
        assert_error::<StorageError>();
    }
}
