//! Lightweight metrics hook for counters and histograms without mandating Prometheus.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Hook for recording operational metrics from eventkit adapters and services.
pub trait MetricsHook: Send + Sync {
    /// Increment a counter by `delta` (default 1).
    fn increment_counter(&self, name: &str, labels: &[(&str, &str)], delta: u64);

    /// Record a histogram/sample observation.
    fn observe_histogram(&self, name: &str, labels: &[(&str, &str)], value: f64);

    /// Set a gauge value.
    fn set_gauge(&self, name: &str, labels: &[(&str, &str)], value: f64);
}

/// No-op metrics implementation for library-only / test usage.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopMetrics;

impl MetricsHook for NoopMetrics {
    fn increment_counter(&self, _name: &str, _labels: &[(&str, &str)], _delta: u64) {}

    fn observe_histogram(&self, _name: &str, _labels: &[(&str, &str)], _value: f64) {}

    fn set_gauge(&self, _name: &str, _labels: &[(&str, &str)], _value: f64) {}
}

/// In-memory counter registry suitable for health endpoints and local diagnostics.
#[derive(Debug, Default)]
pub struct CounterRegistry {
    inner: Mutex<HashMap<String, u64>>,
}

impl CounterRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a snapshot of all counters.
    pub fn snapshot(&self) -> HashMap<String, u64> {
        self.inner.lock().clone()
    }
}

impl MetricsHook for CounterRegistry {
    fn increment_counter(&self, name: &str, _labels: &[(&str, &str)], delta: u64) {
        let mut map = self.inner.lock();
        *map.entry(name.to_string()).or_insert(0) += delta;
    }

    fn observe_histogram(&self, name: &str, _labels: &[(&str, &str)], value: f64) {
        // Store last observed value under a derived key for simplicity.
        let mut map = self.inner.lock();
        map.insert(format!("{name}_last"), value as u64);
    }

    fn set_gauge(&self, name: &str, _labels: &[(&str, &str)], value: f64) {
        let mut map = self.inner.lock();
        map.insert(name.to_string(), value as u64);
    }
}

/// Shared metrics handle for injection into adapters.
pub type SharedMetrics = Arc<dyn MetricsHook>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_registry_accumulates() {
        let reg = CounterRegistry::new();
        reg.increment_counter("events_published", &[], 1);
        reg.increment_counter("events_published", &[], 2);
        assert_eq!(reg.snapshot().get("events_published"), Some(&3));
    }
}
