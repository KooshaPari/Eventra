//! Projection System

use std::collections::HashMap;
use parking_lot::RwLock;
use async_trait::async_trait;

use crate::domain::{Event, EventError, EventHandler, EventStore};

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
        let handles = projection.handles().to_vec();
        drop(self.projections.write().insert(name.clone(), Box::new(projection)));
        drop(self.state.write().insert(name, ProjectionState {
            name,
            position: 0,
            last_updated: chrono::Utc::now(),
        }));
    }

    pub fn run(&self) -> Result<(), EventError> {
        let events = self.event_store.get_events_since(chrono::Utc::now())?;
        let projections = self.projections.read();
        let mut state = self.state.write();

        for event in events {
            for (_, projection) in projections.iter() {
                if projection.handles().contains(&event.metadata.event_type) {
                    if let Some(state) = state.get_mut(projection.name()) {
                        projection.apply(&event)?;
                        state.position += 1;
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
