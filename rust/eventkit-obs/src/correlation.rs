//! Request / trace correlation ID propagation via `tracing` spans.

use std::cell::RefCell;
use uuid::Uuid;

/// Field name used in structured log output for correlation IDs.
pub const CORRELATION_FIELD: &str = "correlation_id";

thread_local! {
    static CURRENT_CORRELATION: RefCell<Option<Uuid>> = const { RefCell::new(None) };
}

/// Return the current thread-local correlation ID, if set.
pub fn correlation_id() -> Option<Uuid> {
    CURRENT_CORRELATION.with(|cell| *cell.borrow())
}

/// Set the correlation ID for the current thread and enter a tracing span.
pub fn set_correlation_id(id: Uuid) {
    CURRENT_CORRELATION.with(|cell| *cell.borrow_mut() = Some(id));
    tracing::Span::current().record(CORRELATION_FIELD, tracing::field::display(id));
}

/// Ensure a correlation ID exists; generate one when absent.
pub fn ensure_correlation_id() -> Uuid {
    if let Some(id) = correlation_id() {
        return id;
    }
    let id = Uuid::new_v4();
    set_correlation_id(id);
    id
}

/// Run `f` inside a span tagged with the given correlation ID.
pub fn with_correlation_id<T, F>(id: Uuid, operation: &str, f: F) -> T
where
    F: FnOnce() -> T,
{
    let span = tracing::info_span!(
        "request",
        correlation_id = %id,
        operation = %operation,
    );
    let _guard = span.enter();
    set_correlation_id(id);
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_generates_and_reuses_id() {
        let first = ensure_correlation_id();
        let second = ensure_correlation_id();
        assert_eq!(first, second);
    }
}
