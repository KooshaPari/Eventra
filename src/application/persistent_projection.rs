//! File-backed persistent projection checkpoint store.
//!
//! Persists [`ProjectionState`] for every projection to disk so that a
//! process restart resumes from the last applied position instead of
//! replaying the entire event log. This is the missing "persistent
//! checkpoint" capability called out by the 71-pillar audit.
//!
//! ## On-disk layout
//!
//! ```text
//! <root>/
//!   <projection_name>.json   # one JSON file per projection
//! ```
//!
//! Each file is a single serialized [`ProjectionState`] record and is
//! rewritten atomically (write to temp + rename) on every update.
//!
//! ## Postgres migration path
//!
//! In a multi-process deployment the checkpoint should live in a shared
//! store. The intended relational shape is a single table:
//!
//! ```sql
//! CREATE TABLE projection_checkpoints (
//!     projection_name TEXT PRIMARY KEY,
//!     position        BIGINT NOT NULL,
//!     last_updated    TIMESTAMPTZ NOT NULL
//! );
//! ```
//!
//! Replacing [`FileCheckpointStore`] with a `PostgresCheckpointStore` only
//! requires implementing the [`CheckpointStore`] trait — the
//! [`CheckpointedProjectionRunner`] consumes it through a trait object.

use std::fs;
use std::path::{Path, PathBuf};

use parking_lot::RwLock;

use crate::application::projection::{Projection, ProjectionState};
use crate::domain::{Event, EventError, EventStore};

/// Port for projection checkpoint persistence.
pub trait CheckpointStore: Send + Sync {
    /// Load the last persisted position for `projection`, or `None` if this
    /// is the first time the projection has run.
    fn load(&self, projection: &str) -> Result<Option<u64>, EventError>;
    /// Persist `position` as the new high-water mark for `projection`.
    fn save(&self, projection: &str, position: u64) -> Result<(), EventError>;
}

/// File-backed [`CheckpointStore`]. Each projection gets its own JSON file
/// inside the configured root directory.
pub struct FileCheckpointStore {
    root: PathBuf,
    cache: RwLock<std::collections::HashMap<String, u64>>,
}

impl FileCheckpointStore {
    /// Open or create a checkpoint store rooted at `root`. The directory
    /// is created if it does not exist.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, EventError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)
            .map_err(|e| EventError::Store(format!("create checkpoint dir: {e}")))?;
        Ok(Self {
            root,
            cache: RwLock::new(std::collections::HashMap::new()),
        })
    }

    fn file_for(&self, projection: &str) -> PathBuf {
        let safe: String = projection
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        self.root.join(format!("{safe}.json"))
    }
}

impl CheckpointStore for FileCheckpointStore {
    fn load(&self, projection: &str) -> Result<Option<u64>, EventError> {
        if let Some(cached) = self.cache.read().get(projection).copied() {
            return Ok(Some(cached));
        }
        let path = self.file_for(projection);
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| EventError::Store(format!("read checkpoint: {e}")))?;
        let state: ProjectionState = serde_json::from_str(&raw)
            .map_err(|e| EventError::Store(format!("parse checkpoint: {e}")))?;
        self.cache.write().insert(projection.to_string(), state.position);
        Ok(Some(state.position))
    }

    fn save(&self, projection: &str, position: u64) -> Result<(), EventError> {
        let path = self.file_for(projection);
        let tmp = path.with_extension("json.tmp");
        let state = ProjectionState {
            name: projection.to_string(),
            position,
            last_updated: chrono::Utc::now(),
        };
        let serialized = serde_json::to_string(&state)?;
        fs::write(&tmp, serialized)
            .map_err(|e| EventError::Store(format!("write checkpoint tmp: {e}")))?;
        fs::rename(&tmp, &path)
            .map_err(|e| EventError::Store(format!("commit checkpoint: {e}")))?;
        self.cache.write().insert(projection.to_string(), position);
        Ok(())
    }
}

/// Projection runner that resumes from a persisted checkpoint and saves
/// progress after every event. This complements
/// [`crate::application::projection::ProjectionRunner`], which is in-memory
/// only.
pub struct CheckpointedProjectionRunner {
    projections: RwLock<std::collections::HashMap<String, Box<dyn Projection>>>,
    event_store: Box<dyn EventStore>,
    checkpoints: Box<dyn CheckpointStore>,
}

impl CheckpointedProjectionRunner {
    /// Construct a new runner over `event_store` using `checkpoints` for
    /// persistence.
    pub fn new(event_store: Box<dyn EventStore>, checkpoints: Box<dyn CheckpointStore>) -> Self {
        Self {
            projections: RwLock::new(std::collections::HashMap::new()),
            event_store,
            checkpoints,
        }
    }

    /// Register a projection with the runner.
    pub fn register<P: Projection + 'static>(&self, projection: P) {
        let name = projection.name().to_string();
        self.projections
            .write()
            .insert(name, Box::new(projection));
    }

    /// Apply every event past the persisted checkpoint and update the
    /// checkpoint after each successful apply. Returns the number of
    /// events applied during this call.
    pub fn run(&self) -> Result<u64, EventError> {
        let events = self.event_store.get_all_events()?;
        let mut projections = self.projections.write();
        let mut applied = 0u64;
        for (idx, event) in events.iter().enumerate() {
            let position = idx as u64;
            for projection in projections.values_mut() {
                if !projection.handles().contains(&event.metadata.event_type) {
                    continue;
                }
                let name = projection.name().to_string();
                let last = self.checkpoints.load(&name)?.unwrap_or(0);
                if position < last {
                    continue;
                }
                projection.apply(event)?;
                self.checkpoints.save(&name, position + 1)?;
                applied += 1;
            }
        }
        Ok(applied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::event_store::InMemoryEventStore;
    use crate::domain::Event;
    use serde_json::json;
    use std::sync::Arc;

    fn tmp_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "eventkit-checkpoint-store-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).expect("create tmp root");
        dir
    }

    struct Counter {
        name: String,
        handled: Vec<String>,
        count: Arc<parking_lot::Mutex<u32>>,
    }

    impl Counter {
        fn new(name: &str, count: Arc<parking_lot::Mutex<u32>>) -> Self {
            Self {
                name: name.to_string(),
                handled: vec!["ping".to_string()],
                count,
            }
        }
    }

    impl Projection for Counter {
        fn name(&self) -> &str {
            &self.name
        }
        fn handles(&self) -> &[String] {
            &self.handled
        }
        fn apply(&mut self, _event: &Event) -> Result<(), EventError> {
            *self.count.lock() += 1;
            Ok(())
        }
    }

    fn store_with_events(count: u32) -> Box<dyn EventStore> {
        let s = InMemoryEventStore::new();
        for v in 1..=count {
            let event = Event::new("a", "Agg", "ping", v, json!({"v": v}));
            s.append(&event).expect("append");
        }
        Box::new(s)
    }

    #[test]
    fn checkpoint_round_trip_through_disk() {
        let root = tmp_root("roundtrip");
        let store = FileCheckpointStore::open(&root).expect("open");
        assert_eq!(store.load("counter").expect("load fresh"), None);
        store.save("counter", 7).expect("save 7");
        let loaded = store.load("counter").expect("load").expect("present");
        assert_eq!(loaded, 7);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn checkpoint_persists_across_store_instances() {
        let root = tmp_root("persist");
        {
            let store = FileCheckpointStore::open(&root).expect("open");
            store.save("counter", 42).expect("save");
        }
        let store = FileCheckpointStore::open(&root).expect("reopen");
        let loaded = store.load("counter").expect("load").expect("present");
        assert_eq!(loaded, 42);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn runner_resumes_from_persisted_checkpoint() {
        let root = tmp_root("runner");
        let count_a = Arc::new(parking_lot::Mutex::new(0));
        let count_b = Arc::new(parking_lot::Mutex::new(0));

        // First run on a store with 5 events: applies 5.
        {
            let runner = CheckpointedProjectionRunner::new(
                store_with_events(5),
                Box::new(FileCheckpointStore::open(&root).expect("open")),
            );
            runner.register(Counter::new("c", count_a.clone()));
            let applied = runner.run().expect("run");
            assert_eq!(applied, 5);
            assert_eq!(*count_a.lock(), 5);
        }

        // Second run: checkpoint is at 5, so nothing is applied.
        {
            let runner = CheckpointedProjectionRunner::new(
                store_with_events(5),
                Box::new(FileCheckpointStore::open(&root).expect("reopen")),
            );
            runner.register(Counter::new("c", count_b.clone()));
            let applied = runner.run().expect("run");
            assert_eq!(applied, 0);
            assert_eq!(*count_b.lock(), 0);
        }

        fs::remove_dir_all(&root).ok();
    }
}
