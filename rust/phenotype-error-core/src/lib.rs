// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Local replacement for the (no-longer-accessible) upstream `phenotype-error-core`
// crate. This crate intentionally exposes only the surface area used by
// `phenotype-event-bus`: `RepositoryError` and `StorageError`, both used as
// `#[from]` sources in `EventBusError`.
//
// See: `cargo: failed to get phenotype-error-core as a dependency ... revision
// dd14f735c373e66b1907b7d5d66d3e175abe1df2 not found` (CI run #28065973912 on
// b50947f).

use thiserror::Error;

/// Storage-layer error. Used as a `#[from]` source for `EventBusError::Storage`.
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("storage I/O error: {0}")]
    Io(String),

    #[error("storage backend unavailable: {0}")]
    Unavailable(String),

    #[error("storage serialization error: {0}")]
    Serialization(String),

    #[error("storage timeout after {0}ms")]
    Timeout(u64),
}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

/// Repository-layer error. Used as a `#[from]` source for `EventBusError::Repository`.
#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("repository not found: {0}")]
    NotFound(String),

    #[error("repository conflict: {0}")]
    Conflict(String),

    #[error("repository I/O error: {0}")]
    Io(String),

    #[error("repository serialization error: {0}")]
    Serialization(String),
}

impl From<std::io::Error> for RepositoryError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<serde_json::Error> for RepositoryError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_error_io_displays_message() {
        let err = StorageError::Io("disk full".into());
        assert_eq!(err.to_string(), "storage I/O error: disk full");
    }

    #[test]
    fn repository_error_not_found_displays_message() {
        let err = RepositoryError::NotFound("events/42".into());
        assert_eq!(err.to_string(), "repository not found: events/42");
    }

    #[test]
    fn from_io_error_for_storage() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope");
        let err: StorageError = io.into();
        assert!(matches!(err, StorageError::Io(_)));
    }
}
