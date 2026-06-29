//! SqliteOutbox: SQLite-backed OutboxStore for embedded/dev/test use.
//!
//! EVE-SOTA-004. Pair with the PostgresOutbox (EVE-SOTA-002) when you need
//! concurrent multi-relay workers; SqliteOutbox is best for single-process
//! dev, CLI tools, and integration tests where spinning up Postgres is
//! overkill.
//!
//! Schema (MIGRATION_SQL):
//!
//! ```sql
//! CREATE TABLE IF NOT EXISTS outbox (
//!     id           BLOB PRIMARY KEY,          -- 16-byte ULID
//!     aggregate_id TEXT NOT NULL,
//!     envelope     TEXT NOT NULL,             -- JSON-serialized EventEnvelope
//!     created_at   INTEGER NOT NULL,          -- unix millis
//!     published_at INTEGER,                   -- unix millis; NULL = pending
//!     attempt      INTEGER NOT NULL DEFAULT 0,
//!     last_error   TEXT
//! );
//! CREATE INDEX IF NOT EXISTS outbox_pending_created_idx
//!   ON outbox (created_at) WHERE published_at IS NULL;
//! ```
//!
//! The pending index is partial — only unpublished rows are tracked, so the
//! index stays small even with millions of historical rows.
//!
//! `claim_batch` uses a SQLite UPDATE ... RETURNING transaction to atomically
//! mark rows as in-flight (attempt++) and return them. This is safe under
//! SQLite's writer-serialization model: only one writer at a time, so we
//! don't need `FOR UPDATE SKIP LOCKED` like Postgres.
//!
//! `transactional(|tx| ...)` opens a deferred transaction so callers can
//! bundle the aggregate mutation + `SqliteOutbox::enqueue_in_tx(tx, entry)`
//! in one atomic commit.

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json;
use std::sync::{Arc, Mutex};
use tracing::{debug, instrument};

use super::{
    aggregate_root, ClaimedBatch, OutboxEntry, OutboxError, OutboxStore,
    OutboxStoreKind, StorageExt,
};

/// Canonical SQLite schema. Apply via `SqliteOutbox::apply_migrations(conn)`
/// or copy into your migration runner.
pub const MIGRATION_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS outbox (
    id           BLOB PRIMARY KEY,
    aggregate_id TEXT NOT NULL,
    envelope     TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    published_at INTEGER,
    attempt      INTEGER NOT NULL DEFAULT 0,
    last_error   TEXT
);
CREATE INDEX IF NOT EXISTS outbox_pending_created_idx
  ON outbox (created_at) WHERE published_at IS NULL;
"#;

/// SQLite-backed OutboxStore.
///
/// Single-process; safe under SQLite's writer-serialization. For multi-
/// writer workloads, use PostgresOutbox (EVE-SOTA-002).
#[derive(Clone)]
pub struct SqliteOutbox {
    inner: Arc<Mutex<Connection>>,
}

impl std::fmt::Debug for SqliteOutbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteOutbox").finish_non_exhaustive()
    }
}

impl SqliteOutbox {
    /// Open an in-memory database (tests, ephemeral dev).
    pub fn open_in_memory() -> Result<Self, OutboxError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| OutboxError::Storage(e.to_string()))?;
        Self::from_connection(conn)
    }

    /// Open a file-backed database at `path`.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, OutboxError> {
        let conn = Connection::open(path)
            .map_err(|e| OutboxError::Storage(e.to_string()))?;
        Self::from_connection(conn)
    }

    /// Wrap an existing `rusqlite::Connection`. Applies the schema
    /// immediately; safe to call repeatedly (CREATE IF NOT EXISTS).
    pub fn from_connection(conn: Connection) -> Result<Self, OutboxError> {
        conn.execute_batch(MIGRATION_SQL)
            .map_err(|e| OutboxError::Storage(format!("apply_migrations: {e}")))?;
        // WAL mode improves concurrency for readers (the relay polls while
        // writers enqueue). safe_mode = normal allows app-level locking.
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "synchronous", "NORMAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON").ok();
        Ok(Self { inner: Arc::new(Mutex::new(conn)) })
    }

    /// Apply the schema to a connection you already manage.
    pub fn apply_migrations(conn: &Connection) -> Result<(), OutboxError> {
        conn.execute_batch(MIGRATION_SQL)
            .map_err(|e| OutboxError::Storage(format!("apply_migrations: {e}")))
    }

    /// Run `f` inside a SQLite transaction. Commits on Ok, rolls back on
    /// Err. Use with `enqueue_in_tx` to bundle aggregate mutation +
    /// outbox insert into one atomic commit.
    ///
    /// ```ignore
    /// SqliteOutbox::transactional(&outbox, |tx| {
    ///     // your aggregate mutation here using tx
    ///     outbox.enqueue_in_tx(tx, entry)?;
    ///     Ok(())
    /// })?;
    /// ```
    pub fn transactional<F, R>(outbox: &SqliteOutbox, f: F) -> Result<R, OutboxError>
    where
        F: FnOnce(&Transaction<'_>) -> Result<R, OutboxError>,
    {
        let mut guard = outbox
            .inner
            .lock()
            .map_err(|e| OutboxError::Storage(format!("mutex poisoned: {e}")))?;
        let tx = guard
            .transaction()
            .map_err(|e| OutboxError::Storage(e.to_string()))?;
        let result = f(&tx)?;
        tx.commit()
            .map_err(|e| OutboxError::Storage(format!("commit: {e}")))?;
        Ok(result)
    }

    /// Enqueue an entry inside an open transaction. Pair with
    /// `SqliteOutbox::transactional` for dual-write safety.
    #[instrument(skip(tx, entry), fields(outbox.id = %entry.id))]
    pub fn enqueue_in_tx(
        &self,
        tx: &Transaction<'_>,
        entry: OutboxEntry,
    ) -> Result<(), OutboxError> {
        let id_bytes = entry.id.to_bytes();
        let envelope_json = serde_json::to_string(&entry.envelope)
            .map_err(|e| OutboxError::Serde(e.to_string()))?;
        let created_at_ms = entry.created_at.timestamp_millis();
        tx.execute(
            "INSERT INTO outbox (id, aggregate_id, envelope, created_at, published_at, attempt, last_error) \
             VALUES (?1, ?2, ?3, ?4, NULL, 0, NULL)",
            params![
                id_bytes.as_slice(),
                entry.aggregate_id,
                envelope_json,
                created_at_ms,
            ],
        )
        .map_err(|e| OutboxError::Storage(format!("enqueue_in_tx: {e}")))?;
        debug!(outbox.id = %entry.id, "enqueue_in_tx committed");
        Ok(())
    }
}

#[async_trait]
impl OutboxStore for SqliteOutbox {
    fn kind(&self) -> OutboxStoreKind {
        OutboxStoreKind::Sqlite
    }

    /// Enqueue an entry in its own implicit transaction. For atomic
    /// aggregate-mutation + outbox writes, use `transactional` +
    /// `enqueue_in_tx` instead.
    #[instrument(skip(self, entry), fields(outbox.id = %entry.id))]
    async fn enqueue(&self, entry: OutboxEntry) -> Result<(), OutboxError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| OutboxError::Storage(format!("mutex poisoned: {e}")))?;
        let tx = guard
            .transaction()
            .map_err(|e| OutboxError::Storage(e.to_string()))?;
        let id_bytes = entry.id.to_bytes();
        let envelope_json = serde_json::to_string(&entry.envelope)
            .map_err(|e| OutboxError::Serde(e.to_string()))?;
        let created_at_ms = entry.created_at.timestamp_millis();
        tx.execute(
            "INSERT INTO outbox (id, aggregate_id, envelope, created_at, published_at, attempt, last_error) \
             VALUES (?1, ?2, ?3, ?4, NULL, 0, NULL)",
            params![
                id_bytes.as_slice(),
                entry.aggregate_id,
                envelope_json,
                created_at_ms,
            ],
        )
        .map_err(|e| OutboxError::Storage(format!("enqueue: {e}")))?;
        tx.commit()
            .map_err(|e| OutboxError::Storage(format!("commit: {e}")))?;
        debug!(outbox.id = %entry.id, "enqueue committed");
        Ok(())
    }

    #[instrument(skip(self), fields(batch_size = batch_size))]
    async fn claim_batch(
        &self,
        batch_size: usize,
    ) -> Result<ClaimedBatch, OutboxError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| OutboxError::Storage(format!("mutex poisoned: {e}")))?;
        let tx = guard
            .transaction()
            .map_err(|e| OutboxError::Storage(e.to_string()))?;

        // Atomically claim oldest pending rows: bump attempt + return rows.
        // WHERE id IN (SELECT id FROM outbox WHERE published_at IS NULL
        //              ORDER BY created_at ASC LIMIT ?)
        let limit = batch_size.max(1) as i64;
        let mut stmt = tx
            .prepare(
                "SELECT id, aggregate_id, envelope, created_at, attempt \
                 FROM outbox \
                 WHERE published_at IS NULL \
                 ORDER BY created_at ASC \
                 LIMIT ?",
            )
            .map_err(|e| OutboxError::Storage(format!("claim_batch prepare: {e}")))?;

        let rows: rusqlite::Result<Vec<(Vec<u8>, String, String, i64, i64)>> = stmt
            .query_map([limit], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect();

        let rows = rows.map_err(|e| OutboxError::Storage(format!("claim_batch query: {e}")))?;

        let mut entries = Vec::with_capacity(rows.len());
        let mut update = tx
            .prepare(
                "UPDATE outbox SET attempt = attempt + 1 WHERE id = ?1",
            )
            .map_err(|e| OutboxError::Storage(format!("claim_batch update prepare: {e}")))?;

        for (id_bytes, aggregate_id, envelope_json, created_at_ms, attempt) in rows {
            let id = ulid::Ulid::from_bytes(id_bytes.as_slice().try_into().map_err(|_| {
                OutboxError::Storage(format!("bad ulid bytes: id={aggregate_id}"))
            })?);
            let envelope: super::EventEnvelope<serde_json::Value> =
                serde_json::from_str(&envelope_json)
                    .map_err(|e| OutboxError::Serde(e.to_string()))?;
            let created_at = Utc
                .timestamp_millis_opt(created_at_ms)
                .single()
                .ok_or_else(|| OutboxError::Storage("bad created_at".into()))?;
            entries.push(OutboxEntry {
                id,
                aggregate_id: aggregate_id.clone(),
                envelope,
                created_at,
                published_at: None,
                attempt: (attempt as u32) + 1,
                last_error: None,
            });
            update
                .execute(params![id_bytes.as_slice()])
                .map_err(|e| OutboxError::Storage(format!("claim_batch update: {e}")))?;
        }
        drop(update);
        drop(stmt);

        tx.commit()
            .map_err(|e| OutboxError::Storage(format!("claim_batch commit: {e}")))?;

        debug!(claimed = entries.len(), "claim_batch committed");
        Ok(ClaimedBatch::new(entries))
    }

    #[instrument(skip(self), fields(outbox.id = %id))]
    async fn mark_published(&self, id: ulid::Ulid) -> Result<(), OutboxError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| OutboxError::Storage(format!("mutex poisoned: {e}")))?;
        let id_bytes = id.to_bytes();
        let now_ms = Utc::now().timestamp_millis();
        let rows = guard
            .execute(
                "UPDATE outbox SET published_at = COALESCE(published_at, ?1) WHERE id = ?2",
                params![now_ms, id_bytes.as_slice()],
            )
            .map_err(|e| OutboxError::Storage(format!("mark_published: {e}")))?;
        if rows == 0 {
            return Err(OutboxError::NotFound(id));
        }
        debug!(outbox.id = %id, "mark_published");
        Ok(())
    }

    #[instrument(skip(self, err), fields(outbox.id = %id))]
    async fn record_failure(
        &self,
        id: ulid::Ulid,
        err: &str,
    ) -> Result<(), OutboxError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| OutboxError::Storage(format!("mutex poisoned: {e}")))?;
        let id_bytes = id.to_bytes();
        let rows = guard
            .execute(
                "UPDATE outbox SET attempt = attempt + 1, last_error = ?1 WHERE id = ?2",
                params![err, id_bytes.as_slice()],
            )
            .map_err(|e| OutboxError::Storage(format!("record_failure: {e}")))?;
        if rows == 0 {
            return Err(OutboxError::NotFound(id));
        }
        debug!(outbox.id = %id, "record_failure");
        Ok(())
    }

    #[instrument(skip(self))]
    async fn pending_count(&self) -> Result<u64, OutboxError> {
        let guard = self
            .inner
            .lock()
            .map_err(|e| OutboxError::Storage(format!("mutex poisoned: {e}")))?;
        let count: i64 = guard
            .query_row(
                "SELECT COUNT(*) FROM outbox WHERE published_at IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(|e| OutboxError::Storage(format!("pending_count: {e}")))?;
        Ok(count.max(0) as u64)
    }
}

// Surface aggregate_root from super; not used here but re-exported for the
// "OutboxStore trait lives in lib.rs" pattern. This silences the unused
// import warning.
#[allow(dead_code)]
fn _ensure_super_re_exports() {
    let _ = aggregate_root;
}
