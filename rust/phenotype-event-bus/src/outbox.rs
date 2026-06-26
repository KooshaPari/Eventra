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

use super::EventEnvelope;

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
    pub fn new(
        aggregate_id: impl Into<String>,
        envelope: EventEnvelope<serde_json::Value>,
    ) -> Self {
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
        let row = self
            .rows
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(OutboxError::NotFound(id))?;
        if row.published_at.is_none() {
            row.published_at = Some(Utc::now());
        }
        Ok(())
    }

    async fn record_failure(&mut self, id: Ulid, err: &str) -> Result<(), OutboxError> {
        let row = self
            .rows
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(OutboxError::NotFound(id))?;
        row.attempt = row.attempt.saturating_add(1);
        row.last_error = Some(err.to_string());
        Ok(())
    }

    async fn pending_count(&self) -> Result<u64, OutboxError> {
        Ok(self
            .rows
            .iter()
            .filter(|r| r.published_at.is_none())
            .count() as u64)
    }
}

/// Postgres outbox adapter (EVE-SOTA-002). Uses
/// `SELECT ... FOR UPDATE SKIP LOCKED` so multiple relay workers can
/// safely operate against the same table. Requires the `postgres`
/// feature (which pulls in `sqlx`).
///
/// ## Schema
///
/// The relay assumes this table exists (created via a migration):
///
/// ```sql
/// CREATE TABLE IF NOT EXISTS outbox (
///     id            BYTEA       PRIMARY KEY,        -- ULID as 16 raw bytes
///     aggregate_id  TEXT        NOT NULL,
///     envelope      JSONB       NOT NULL,
///     created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
///     published_at  TIMESTAMPTZ,
///     attempt       INT         NOT NULL DEFAULT 0,
///     last_error    TEXT
/// );
/// CREATE INDEX IF NOT EXISTS outbox_unpublished_idx
///     ON outbox (created_at)
///     WHERE published_at IS NULL;
/// ```
///
/// `enqueue` MUST be called inside the same SQL transaction as the
/// aggregate mutation. `PostgresOutbox::transactional` is a helper for
/// that.
#[cfg(feature = "postgres")]
pub mod postgres {
    //! Postgres-backed outbox.

    use super::*;
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use sqlx::{Postgres, Row};
    use std::time::Duration;

    /// Postgres-backed outbox. Cheap to clone (wraps `PgPool`).
    #[derive(Clone)]
    pub struct PostgresOutbox {
        pool: sqlx::PgPool,
    }

    impl std::fmt::Debug for PostgresOutbox {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("PostgresOutbox").finish_non_exhaustive()
        }
    }

    impl PostgresOutbox {
        /// Build a `PostgresOutbox` from a `sqlx::PgPool`.
        pub fn new(pool: sqlx::PgPool) -> Self {
            Self { pool }
        }

        /// Build a `PostgresOutbox` from a libpq URL. Blocks until the
        /// pool is healthy or `connect_timeout` elapses.
        pub async fn connect(url: &str) -> Result<Self, OutboxError> {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(16)
                .acquire_timeout(Duration::from_secs(5))
                .connect(url)
                .await
                .map_err(|e| OutboxError::Database(e.to_string()))?;
            Ok(Self { pool })
        }

        /// Helper: run a closure inside a Postgres transaction. Use
        /// this from your domain code so the aggregate mutation and
        /// the `outbox.enqueue` happen in the same transaction.
        ///
        /// ```ignore
        /// ob.transactional(|mut tx| async move {
        ///     aggregate_repo.save(&mut tx, agg).await?;
        ///     outbox::OutboxEntry::new(agg.id(), envelope())
        ///         .enqueue_in_tx(&mut tx)
        ///         .await?;
        ///     Ok(())
        /// }).await?;
        /// ```
        pub async fn transactional<F, Fut, T>(&self, f: F) -> Result<T, OutboxError>
        where
            F: for<'c> FnOnce(&'c mut sqlx::Transaction<'_, Postgres>) -> Fut,
            Fut: std::future::Future<Output = Result<T, OutboxError>>,
        {
            let mut tx = self
                .pool
                .begin()
                .await
                .map_err(|e| OutboxError::Database(e.to_string()))?;
            let out = f(&mut tx).await?;
            tx.commit()
                .await
                .map_err(|e| OutboxError::Database(e.to_string()))?;
            Ok(out)
        }

        /// Enqueue an entry using a transaction the caller already
        /// holds. This is the dual-write-safe insertion point.
        pub async fn enqueue_in_tx(
            tx: &mut sqlx::Transaction<'_, Postgres>,
            entry: OutboxEntry,
        ) -> Result<(), OutboxError> {
            let id_bytes = entry.id.to_bytes();
            let aggregate_id = entry.aggregate_id.as_str();
            let envelope_json = serde_json::to_value(&entry.envelope)
                .map_err(|e| OutboxError::Serde(e.to_string()))?;
            let created_at = entry.created_at;
            sqlx::query(
                r#"
                INSERT INTO outbox
                    (id, aggregate_id, envelope, created_at, published_at, attempt, last_error)
                VALUES ($1, $2, $3, $4, NULL, 0, NULL)
                "#,
            )
            .bind(&id_bytes[..])
            .bind(aggregate_id)
            .bind(envelope_json)
            .bind(created_at)
            .execute(&mut **tx)
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;
            Ok(())
        }
    }
    fn map_err(e: sqlx::Error) -> OutboxError {
        OutboxError::Database(e.to_string())
    }

    fn parse_entry(row: sqlx::postgres::PgRow) -> Result<OutboxEntry, OutboxError> {
        let id_bytes: Vec<u8> = row.try_get("id").map_err(map_err)?;
        if id_bytes.len() != 16 {
            return Err(OutboxError::Storage(format!(
                "outbox.id must be 16 bytes, got {}",
                id_bytes.len()
            )));
        }
        let mut arr = [0u8; 16];
        arr.copy_from_slice(&id_bytes);
        let ulid = Ulid::from_bytes(arr);

        let aggregate_id: String = row.try_get("aggregate_id").map_err(map_err)?;
        let envelope_json: serde_json::Value = row.try_get("envelope").map_err(map_err)?;
        let envelope: EventEnvelope<serde_json::Value> =
            serde_json::from_value(envelope_json).map_err(|e| OutboxError::Serde(e.to_string()))?;
        let created_at: DateTime<Utc> = row.try_get("created_at").map_err(map_err)?;
        let published_at: Option<DateTime<Utc>> = row.try_get("published_at").map_err(map_err)?;
        let attempt: i32 = row.try_get("attempt").map_err(map_err)?;
        let last_error: Option<String> = row.try_get("last_error").map_err(map_err)?;

        Ok(OutboxEntry {
            id: ulid,
            aggregate_id,
            envelope,
            created_at,
            published_at,
            attempt: attempt.max(0) as u32,
            last_error,
        })
    }

    #[async_trait]
    impl OutboxStore for PostgresOutbox {
        async fn enqueue(&mut self, entry: OutboxEntry) -> Result<(), OutboxError> {
            self.transactional(|tx| async move { Self::enqueue_in_tx(tx, entry).await })
                .await
        }

        async fn claim_batch(&mut self, limit: usize) -> Result<Vec<OutboxEntry>, OutboxError> {
            // SKIP LOCKED keeps concurrent relay workers from blocking
            // on the same row.
            let rows = sqlx::query(
                r#"
                UPDATE outbox
                SET attempt = attempt + 1
                WHERE id IN (
                    SELECT id FROM outbox
                    WHERE published_at IS NULL
                    ORDER BY created_at ASC
                    LIMIT $1
                    FOR UPDATE SKIP LOCKED
                )
                RETURNING id, aggregate_id, envelope, created_at,
                          published_at, attempt, last_error
                "#,
            )
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(map_err)?;

            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                out.push(parse_entry(row)?);
            }
            Ok(out)
        }

        async fn mark_published(&mut self, id: Ulid) -> Result<(), OutboxError> {
            let id_bytes = id.to_bytes();
            let res = sqlx::query(
                r#"
                UPDATE outbox
                SET published_at = COALESCE(published_at, now())
                WHERE id = $1
                "#,
            )
            .bind(&id_bytes[..])
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
            if res.rows_affected() == 0 {
                return Err(OutboxError::NotFound(id));
            }
            Ok(())
        }

        async fn record_failure(&mut self, id: Ulid, err: &str) -> Result<(), OutboxError> {
            let id_bytes = id.to_bytes();
            let res = sqlx::query(
                r#"
                UPDATE outbox
                SET attempt = attempt + 1, last_error = $2
                WHERE id = $1
                "#,
            )
            .bind(&id_bytes[..])
            .bind(err)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
            if res.rows_affected() == 0 {
                return Err(OutboxError::NotFound(id));
            }
            Ok(())
        }

        async fn pending_count(&self) -> Result<u64, OutboxError> {
            let row =
                sqlx::query("SELECT COUNT(*)::BIGINT AS c FROM outbox WHERE published_at IS NULL")
                    .fetch_one(&self.pool)
                    .await
                    .map_err(map_err)?;
            let c: i64 = row.try_get("c").map_err(map_err)?;
            Ok(c.max(0) as u64)
        }
    }

    /// Migration helper (call from your `sqlx::migrate!` macro or
    /// embed as `refinery`/`barrel` migration).
    pub const MIGRATION_SQL: &str = r#"
        CREATE TABLE IF NOT EXISTS outbox (
            id            BYTEA       PRIMARY KEY,
            aggregate_id  TEXT        NOT NULL,
            envelope      JSONB       NOT NULL,
            created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
            published_at  TIMESTAMPTZ,
            attempt       INT         NOT NULL DEFAULT 0,
            last_error    TEXT
        );
        CREATE INDEX IF NOT EXISTS outbox_unpublished_idx
            ON outbox (created_at)
            WHERE published_at IS NULL;
    "#;
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
    async fn outbox_entry_id_is_unique_per_call() {
        let e1 = OutboxEntry::new("a", envelope());
        let e2 = OutboxEntry::new("a", envelope());
        assert_ne!(e1.id, e2.id);
    }
}

/// Integration tests for the Postgres outbox adapter. Requires:
///   - `TEST_DATABASE_URL` env var set to a reachable Postgres URL
///   - the schema applied (the test does `MIGRATION_SQL`)
///   - the `postgres` feature enabled
///
/// Run with: `cargo test --features postgres -- --ignored`
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests {
    use super::postgres::PostgresOutbox;
    use super::*;
    use serde_json::json;
    use std::env;

    fn envelope() -> EventEnvelope<serde_json::Value> {
        EventEnvelope::new("test-source", json!({"k": "v"}))
    }

    async fn connect() -> Option<PostgresOutbox> {
        let url = env::var("TEST_DATABASE_URL").ok()?;
        let ob = PostgresOutbox::connect(&url).await.ok()?;
        sqlx::raw_sql(PostgresOutbox::MIGRATION_SQL)
            .execute(&ob.pool_for_test())
            .await
            .ok()?;
        Some(ob)
    }

    #[tokio::test]
    #[ignore]
    async fn pg_enqueue_claim_mark_published() {
        let Some(mut ob) = connect().await else {
            eprintln!("skip: TEST_DATABASE_URL not set");
            return;
        };
        let e = OutboxEntry::new("agg-1", envelope());
        let id = e.id;
        ob.enqueue(e).await.unwrap();
        let claimed = ob.claim_batch(10).await.unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].id, id);
        assert_eq!(claimed[0].attempt, 1);
        ob.mark_published(id).await.unwrap();
        ob.mark_published(id).await.unwrap(); // idempotent
        assert_eq!(ob.pending_count().await.unwrap(), 0);
    }

    #[tokio::test]
    #[ignore]
    async fn pg_record_failure_increments_attempt() {
        let Some(mut ob) = connect().await else {
            eprintln!("skip: TEST_DATABASE_URL not set");
            return;
        };
        let e = OutboxEntry::new("agg-2", envelope());
        let id = e.id;
        ob.enqueue(e).await.unwrap();
        ob.record_failure(id, "boom").await.unwrap();
        let claimed = ob.claim_batch(10).await.unwrap();
        assert_eq!(claimed[0].attempt, 2);
        assert_eq!(claimed[0].last_error.as_deref(), Some("boom"));
    }

    #[tokio::test]
    #[ignore]
    async fn pg_claim_batch_respects_limit() {
        let Some(mut ob) = connect().await else {
            eprintln!("skip: TEST_DATABASE_URL not set");
            return;
        };
        for i in 0..5 {
            ob.enqueue(OutboxEntry::new(format!("agg-{i}"), envelope()))
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let claimed = ob.claim_batch(3).await.unwrap();
        assert_eq!(claimed.len(), 3);
    }

    #[tokio::test]
    #[ignore]
    async fn pg_mark_published_unknown_id_returns_not_found() {
        let Some(mut ob) = connect().await else {
            eprintln!("skip: TEST_DATABASE_URL not set");
            return;
        };
        let bogus = Ulid::new();
        let err = ob.mark_published(bogus).await.unwrap_err();
        assert!(matches!(err, OutboxError::NotFound(id) if id == bogus));
    }
}

// `pool_for_test` is a small escape hatch for the integration tests
// only — production code should hold the pool behind the
// PostgresOutbox struct.
#[cfg(all(test, feature = "postgres"))]
impl super::postgres::PostgresOutbox {
    pub fn pool_for_test(&self) -> sqlx::PgPool {
        self.pool.clone()
    }
}
