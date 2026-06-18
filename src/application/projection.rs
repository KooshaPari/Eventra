//! Projection System

use std::collections::HashMap;
use parking_lot::RwLock;

use crate::domain::{Event, EventError, EventStore};

/// Projection definition
pub trait Projection: Send + Sync {
    fn name(&self) -> &str;
    fn handles(&self) -> &[String];
    fn apply(&mut self, event: &Event) -> Result<(), EventError>;
}

/// Projection state
#[derive(Debug, Clone)]
pub struct ProjectionState {
    pub name: String,
    pub position: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Projection runner
pub struct ProjectionRunner {
    projections: RwLock<HashMap<String, Box<dyn Projection>>>,
    event_store: Box<dyn EventStore>,
    state: RwLock<HashMap<String, ProjectionState>>,
}

impl ProjectionRunner {
    pub fn new(event_store: Box<dyn EventStore>) -> Self {
        Self {
            projections: RwLock::new(HashMap::new()),
            event_store,
            state: RwLock::new(HashMap::new()),
        }
    }

    pub fn register<P: Projection + 'static>(&self, projection: P) {
        let name = projection.name().to_string();
        drop(self.projections.write().insert(name.clone(), Box::new(projection)));
        drop(self.state.write().insert(name.clone(), ProjectionState {
            name,
            position: 0,
            last_updated: chrono::Utc::now(),
        }));
    }

    /// Replay events for every registered projection starting from its saved
    /// position (or from offset 0 for first-time runs). Each projection tracks
    /// its own position via [`ProjectionState`].
    pub fn run(&self) -> Result<(), EventError> {
        let events = self.event_store.get_all_events()?;
        let mut projections = self.projections.write();
        let mut state = self.state.write();

        for (idx, event) in events.iter().enumerate() {
            for (_, projection) in projections.iter_mut() {
                if projection.handles().contains(&event.metadata.event_type) {
                    if let Some(state) = state.get_mut(projection.name()) {
                        let position = idx as u64;
                        if position < state.position {
                            continue;
                        }
                        projection.apply(event)?;
                        state.position = position + 1;
                        state.last_updated = chrono::Utc::now();
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_state(&self, name: &str) -> Option<ProjectionState> {
        self.state.read().get(name).cloned()
    }
}