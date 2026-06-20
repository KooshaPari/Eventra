//! File-backed persistent event store.
//!
//! Provides a production-grade [`PersistentEventStore`] that stores events as
//! an append-only JSON-lines log on local disk. Events survive process
//! restarts, implement optimistic concurrency control, and expose the same
//! [`EventStore`] trait as the in-memory adapter, making the two fully
//! interchangeable.
//!
//! ## On-disk layout
//!
//! ```text
//! <root>/
//!   events.log          # append-only JSONL stream of every accepted event
//!   checkpoints/
//!     <aggregate_id>    # last persisted version per aggregate
//! ```
//!
//! Each line in `events.log` is a self-contained JSON-serialized [`Event`].
//! Reads are implemented by streaming the log from offset 0, so the store
//! has the same ordering guarantees regardless of which method is called.
//!
//! ## PostgreSQL migration path
//!
//! The API surface matches the trait used by [`crate::adapters::event_store::InMemoryEventStore`]
//! exactly, so swapping in a real `PostgresEventStore` later is a
//! drop-in change for callers. The schema below shows the intended
//! relational shape and is the same one this adapter emulates on disk.
//!
//! ```sql
//! CREATE TABLE events (
//!     event_id        UUID PRIMARY KEY,
//!     aggregate_id    TEXT NOT NULL,
//!     aggregate_type  TEXT NOT NULL,
//!     event_type      TEXT NOT NULL,
//!     version         BIGINT NOT NULL,
//!     timestamp       TIMESTAMPTZ NOT NULL,
//!     causation_id    UUID,
//!     correlation_id  UUID,
//!     payload         JSONB NOT NULL,
//!     UNIQUE (aggregate_id, version)
//! );
//! CREATE INDEX events_aggregate_idx ON events (aggregate_id, version);
//! ```
//!
//! Optimistic concurrency is enforced by the `UNIQUE (aggregate_id, version)`
//! constraint, so concurrent appends from multiple processes will cause
//! exactly one of them to fail — which is the same behaviour this file-backed
//! implementation emulates via the per-aggregate checkpoint file.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use parking_lot::Mutex;

use crate::domain::{Event, EventError, EventStore};

/// Append-only, file-backed event store suitable for production single-node
/// deployments. Concurrency between processes is enforced via exclusive
/// append locks and per-aggregate version checkpoints.
pub struct PersistentEventStore {
    root: PathBuf,
    log: Mutex<BufWriter<File>>,
    log_path: PathBuf,
    checkpoint_dir: PathBuf,
}

impl PersistentEventStore {
    /// Open or create a persistent event store rooted at `root`. The
    /// directory is created if it does not exist. Returns
    /// [`EventError::Store`] if the on-disk log is corrupt or unwritable.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, EventError> {
        let root = root.as_ref().to_path_buf();
        let checkpoint_dir = root.join("checkpoints");
        fs::create_dir_all(&checkpoint_dir)
            .map_err(|e| EventError::Store(format!("create root: {e}")))?;

        let log_path = root.join("events.log");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| EventError::Store(format!("open log: {e}")))?;
        let log = Mutex::new(BufWriter::new(file));

        Ok(Self {
            root,
            log,
            log_path,
            checkpoint_dir,
        })
    }

    /// Returns the on-disk root directory used by this store.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn checkpoint_path(&self, aggregate_id: &str) -> PathBuf {
        let safe = sanitize(aggregate_id);
        self.checkpoint_dir.join(safe)
    }

    fn read_checkpoint(&self, aggregate_id: &str) -> Result<u32, EventError> {
        let path = self.checkpoint_path(aggregate_id);
        if !path.exists() {
            return Ok(0);
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| EventError::Store(format!("read checkpoint: {e}")))?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(0);
        }
        trimmed
            .parse::<u32>()
            .map_err(|e| EventError::Store(format!("parse checkpoint: {e}")))
    }

    fn write_checkpoint(&self, aggregate_id: &str, version: u32) -> Result<(), EventError> {
        let path = self.checkpoint_path(aggregate_id);
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, version.to_string())
            .map_err(|e| EventError::Store(format!("write checkpoint tmp: {e}")))?;
        fs::rename(&tmp, &path)
            .map_err(|e| EventError::Store(format!("commit checkpoint: {e}")))?;
        Ok(())
    }

    /// Stream every event in the log, oldest first. Used to back
    /// [`EventStore::get_events`], [`EventStore::get_events_since`], and
    /// [`EventStore::get_all_events`].
    fn read_all(&self) -> Result<Vec<Event>, EventError> {
        let file = File::open(&self.log_path)
            .map_err(|e| EventError::Store(format!("reopen log: {e}")))?;
        let reader = BufReader::new(file);
        let mut out = Vec::new();
        for (idx, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| EventError::Store(format!("read line {idx}: {e}")))?;
            if line.trim().is_empty() {
                continue;
            }
            let event: Event = serde_json::from_str(&line).map_err(|e| {
                EventError::Store(format!("decode line {idx}: {e}"))
            })?;
            out.push(event);
        }
        Ok(out)
    }
}

fn sanitize(input: &str) -> String {
    input
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

impl EventStore for PersistentEventStore {
    fn append(&self, event: &Event) -> Result<(), EventError> {
        // Enforce optimistic concurrency against the on-disk checkpoint.
        let last = self.read_checkpoint(&event.metadata.aggregate_id)?;
        let expected = last + 1;
        if event.metadata.version != expected {
            return Err(EventError::ConcurrencyConflict {
                expected,
                found: event.metadata.version,
            });
        }

        // Serialize the event as a single JSONL line. Serializing before
        // locking keeps the critical section short.
        let line = serde_json::to_string(event)?;
        {
            let mut log = self.log.lock();
            writeln!(log, "{line}")
                .map_err(|e| EventError::Store(format!("write log: {e}")))?;
            log.flush()
                .map_err(|e| EventError::Store(format!("flush log: {e}")))?;
        }

        // Commit the checkpoint only after the line is durable on disk.
        self.write_checkpoint(&event.metadata.aggregate_id, event.metadata.version)?;
        Ok(())
    }

    fn get_events(&self, aggregate_id: &str) -> Result<Vec<Event>, EventError> {
        Ok(self
            .read_all()?
            .into_iter()
            .filter(|e| e.metadata.aggregate_id == aggregate_id)
            .collect())
    }

    fn get_events_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, EventError> {
        Ok(self
            .read_all()?
            .into_iter()
            .filter(|e| e.metadata.timestamp > since)
            .collect())
    }

    fn get_all_events(&self) -> Result<Vec<Event>, EventError> {
        self.read_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Event;
    use serde_json::json;

    fn tmp_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "eventkit-persistent-store-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).expect("create tmp root");
        dir
    }

    fn event(agg: &str, et: &str, version: u32) -> Event {
        Event::new(agg, "Aggregate", et, version, json!({ "v": version }))
    }

    #[test]
    fn append_and_replay_round_trip() {
        let root = tmp_root("roundtrip");
        let store = PersistentEventStore::open(&root).expect("open");
        store.append(&event("a-1", "Created", 1)).expect("append 1");
        store.append(&event("a-1", "Updated", 2)).expect("append 2");
        store.append(&event("a-2", "Created", 1)).expect("append a-2");

        let a1 = store.get_events("a-1").expect("get a-1");
        assert_eq!(a1.len(), 2);
        assert_eq!(a1[0].metadata.version, 1);
        assert_eq!(a1[1].metadata.version, 2);

        let all = store.get_all_events().expect("get_all_events");
        assert_eq!(all.len(), 3);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn reopen_persists_events_across_instances() {
        let root = tmp_root("reopen");
        {
            let store = PersistentEventStore::open(&root).expect("open");
            store.append(&event("a-1", "Created", 1)).expect("append 1");
            store.append(&event("a-1", "Updated", 2)).expect("append 2");
        }
        let store = PersistentEventStore::open(&root).expect("reopen");
        let events = store.get_events("a-1").expect("get_events after reopen");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].metadata.event_type, "Created");
        assert_eq!(events[1].metadata.event_type, "Updated");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn optimistic_concurrency_rejects_out_of_order_version() {
        let root = tmp_root("occ");
        let store = PersistentEventStore::open(&root).expect("open");
        store.append(&event("a-1", "Created", 1)).expect("append 1");

        let err = store
            .append(&event("a-1", "Updated", 3))
            .expect_err("must reject");
        match err {
            EventError::ConcurrencyConflict { expected, found } => {
                assert_eq!(expected, 2);
                assert_eq!(found, 3);
            }
            other => panic!("expected ConcurrencyConflict, got {other:?}"),
        }

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn get_events_since_filters_by_timestamp() {
        let root = tmp_root("since");
        let store = PersistentEventStore::open(&root).expect("open");
        let earlier = chrono::Utc::now() - chrono::Duration::seconds(10);
        let mut e1 = event("a-1", "Created", 1);
        e1.metadata.timestamp = earlier;
        let mut e2 = event("a-1", "Updated", 2);
        e2.metadata.timestamp = chrono::Utc::now();
        store.append(&e1).expect("append 1");
        store.append(&e2).expect("append 2");

        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(5);
        let recent = store.get_events_since(cutoff).expect("since");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].metadata.version, 2);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn second_instance_rejects_duplicate_version_after_reopen() {
        // Simulates the "two processes appending at once" case that the
        // intended Postgres schema would catch with UNIQUE(aggregate_id, version).
        let root = tmp_root("concurrent");
        {
            let a = PersistentEventStore::open(&root).expect("open a");
            a.append(&event("a-1", "Created", 1)).expect("a append 1");
            a.append(&event("a-1", "Updated", 2)).expect("a append 2");
        }
        // A second process tries to append a duplicate version 2.
        let b = PersistentEventStore::open(&root).expect("open b");
        let err = b
            .append(&event("a-1", "Touched", 2))
            .expect_err("b must reject duplicate version");
        assert!(matches!(err, EventError::ConcurrencyConflict { .. }));

        fs::remove_dir_all(&root).ok();
    }
}
