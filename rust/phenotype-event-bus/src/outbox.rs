//! Transactional Outbox pattern (EVE-SOTA-001).
//!
//! Solves the dual-write problem: a domain mutation and a side-effect
//! (publishing an event to the bus) must both happen, or neither does.
//! Instead of writing the event to the bus in the same transaction as
//! the aggregate, we write it to a local outbox table within the same
//! DB transaction. A separate `OutboxRelay` reads unpublished rows and
//! publishes them to the bus, with at-least-once semantics.
//!
//! Consumers must be idempotent (keyed on `OutboxEntry::id`).
//!
//! See EVE-SOTA-001 in the DAG, plus the linked plan `_EVE_SOTA_PLAN.md`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ulid::Ulid;

use super::{EventEnvelope, EventId};

/// A row in the outbox table. The `id` is the dedup key for consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxEntry {
    /// Outbox row id (ULID). Stable across retries; consumer dedup key.
    pub id: Ulid,
    /// Aggregate id this event belongs to.
    pub aggregate_id: String,
    /// The envelope to be published once the relay picks the row up.
    pub envelope: EventEnvelope<serde_json::Value>,
    /// Wall-clock time the row was inserted.
    pub created_at: DateTime<Utc>,
    /// Set to Some when the row has been published at least once.
    pub published_at: Option<DateTime<Utc>>,
    /// Attempt count (incremented on each relay try; >=1 means it has been seen).
    pub attempt: u32,
    /// Optional last error from a failed publish (for diagnostics / dead-letter).
    pub last_error: Option<String>,
}

impl OutboxEntry {
    pub fn new(aggregate_id: impl Into<String>, envelope: EventEnvelope<serde_json::Value>) -> Self {
        Self {
            id: Ulid::new(),
            aggregate_id: aggregate_id.into(),
            envelope,
            created_at: Utc::now(),
            published_at: None,
            attempt: 0,
            last_error: None,
        }
    }
}

/// Errors that can occur in the outbox subsystem.
#[derive(Debug, Error)]
pub enum OutboxError {
    #[error("Outbox store I/O error: {0}")]
    Io(String),
    #[error("Outbox row not found: {0}")]
    NotFound(Ulid),
    #[error("Serialization error: {0}")]
    Serde(String),
    #[error("Storage backend error: {0}")]
    Storage(String),
    #[error("Database error: {0}")]
    Database(String),
}

/// Storage backend for the outbox. Implementations must:
/// 1. Be transactional: `enqueue` must be called within the same DB
///    transaction as the aggregate mutation. The trait does not enforce
///    this; callers must use a `Transaction` or `UnitOfWork` pattern.
/// 2. Be idempotent on `mark_published(id)`: re-marking is a no-op.
/// 3. Return entries in `claim_batch` strictly in ascending `created_at`
///    order so each consumer sees a stable sequence.
#[async_trait]
pub trait OutboxStore: Send + Sync {
    /// Append a new entry. Called inside the same transaction as the
    /// aggregate mutation.
    async fn enqueue(&mut self, entry: OutboxEntry) -> Result<(), OutboxError>;

    /// Claim up to `limit` unpublished entries, oldest first. The relay
    /// MUST be the only caller; backends may use `SELECT ... FOR UPDATE
    /// SKIP LOCKED` semantics to support multiple relay workers.
    async fn claim_batch(&mut self, limit: usize) -> Result<Vec<OutboxEntry>, OutboxError>;

    /// Mark an entry as successfully published. Idempotent.
    async fn mark_published(&mut self, id: Ulid) -> Result<(), OutboxError>;

    /// Record a failed publish attempt and the error string. Bumps
    /// `attempt` counter; does not change `published_at`.
    async fn record_failure(&mut self, id: Ulid, err: &str) -> Result<(), OutboxError>;

    /// Number of rows currently unpublished (for metrics / health).
    async fn pending_count(&self) -> Result<u64, OutboxError>;
}

/// In-memory outbox for tests and single-process dev. Not durable.
#[derive(Debug, Default)]
pub struct InMemoryOutbox {
    rows: Vec<OutboxEntry>,
}

impl InMemoryOutbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[async_trait]
impl OutboxStore for InMemoryOutbox {
    async fn enqueue(&mut self, entry: OutboxEntry) -> Result<(), OutboxError> {
        self.rows.push(entry);
        Ok(())
    }

    async fn claim_batch(&mut self, limit: usize) -> Result<Vec<OutboxEntry>, OutboxError> {
        let mut out = Vec::with_capacity(limit);
        for row in self.rows.iter_mut() {
            if row.published_at.is_none() && out.len() < limit {
                row.attempt += 1;
                out.push(row.clone());
            }
        }
        out.sort_by_key(|e| e.created_at);
        Ok(out)
    }

    async fn mark_published(&mut self, id: Ulid) -> Result<(), OutboxError> {
        let row = self.rows.iter_mut().find(|r| r.id == id).ok_or(OutboxError::NotFound(id))?;
        if row.published_at.is_none() {
            row.published_at = Some(Utc::now());
        }
        Ok(())
    }

    async fn record_failure(&mut self, id: Ulid, err: &str) -> Result<(), OutboxError> {
        let row = self.rows.iter_mut().find(|r| r.id == id).ok_or(OutboxError::NotFound(id))?;
        row.attempt = row.attempt.saturating_add(1);
        row.last_error = Some(err.to_string());
        Ok(())
    }

    async fn pending_count(&self) -> Result<u64, OutboxError> {
        Ok(self.rows.iter().filter(|r| r.published_at.is_none()).count() as u64)
    }
}

/// Postgres outbox (feature-gated; implementation is the follow-up
/// EVE-SOTA-002 unit; this stub documents the contract).
#[cfg(feature = "postgres")]
pub mod postgres {
    //! Postgres-backed outbox. Uses `SELECT ... FOR UPDATE SKIP LOCKED`
    //! for safe multi-relay-worker operation. Requires the `sqlx` crate
    //! (added in EVE-SOTA-002).

    use super::*;
    use async_trait::async_trait;

    /// Placeholder; the real implementation is EVE-SOTA-002.
    pub struct PostgresOutbox {
        #[allow(dead_code)]
        pool: sqlx::PgPool,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn envelope() -> EventEnvelope<serde_json::Value> {
        EventEnvelope::new("test-source", json!({"k": "v"}))
    }

    #[tokio::test]
    async fn in_memory_enqueue_and_claim() {
        let mut ob = InMemoryOutbox::new();
        let e = OutboxEntry::new("agg-1", envelope());
        let id = e.id;
        ob.enqueue(e).await.unwrap();
        assert_eq!(ob.pending_count().await.unwrap(), 1);
        let claimed = ob.claim_batch(10).await.unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, id);
        assert_eq!(claimed[0].attempt, 1);
    }

    #[tokio::test]
    async fn in_memory_mark_published_idempotent() {
        let mut ob = InMemoryOutbox::new();
        let e = OutboxEntry::new("agg-1", envelope());
        let id = e.id;
        ob.enqueue(e).await.unwrap();
        ob.mark_published(id).await.unwrap();
        ob.mark_published(id).await.unwrap();
        assert_eq!(ob.pending_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn in_memory_record_failure_increments_attempt() {
        let mut ob = InMemoryOutbox::new();
        let e = OutboxEntry::new("agg-1", envelope());
        let id = e.id;
        ob.enqueue(e).await.unwrap();
        ob.record_failure(id, "boom").await.unwrap();
        let claimed = ob.claim_batch(10).await.unwrap();
        assert_eq!(claimed[0].attempt, 2);
        assert_eq!(claimed[0].last_error.as_deref(), Some("boom"));
    }

    #[tokio::test]
    async fn claim_batch_respects_limit_and_orders_by_created_at() {
        let mut ob = InMemoryOutbox::new();
        for i in 0..5 {
            let e = OutboxEntry::new(format!("agg-{i}"), envelope());
            ob.enqueue(e).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        let claimed = ob.claim_batch(3).await.unwrap();
        assert_eq!(claimed.len(), 3);
        // Claim_batch is ordered; ascending created_at.
        for w in claimed.windows(2) {
            assert!(w[0].created_at <= w[1].created_at);
        }
    }

    #[tokio::test]
    fn outbox_entry_id_is_unique_per_call() {
        let e1 = OutboxEntry::new("a", envelope());
        let e2 = OutboxEntry::new("a", envelope());
        assert_ne!(e1.id, e2.id);
    }
}
