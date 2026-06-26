//! Observability utilities for the Eventra / eventkit ecosystem.
//!
//! Provides structured logging setup, request/trace correlation IDs,
//! a lightweight metrics hook, and health/readiness probe types.
//!
//! Wire this crate into the workspace (see `docs/remediation/OBSERVABILITY.md`)
//! or depend on it directly from service binaries.

#![warn(missing_docs)]

pub mod correlation;
pub mod health;
pub mod logging;
pub mod metrics;

#[cfg(feature = "http-health")]
pub mod http_health;

pub use correlation::{correlation_id, ensure_correlation_id, set_correlation_id, CORRELATION_FIELD};
pub use health::{HealthReport, HealthStatus, Probe, ReadinessReport};
pub use logging::{init_logging, LogFormat, LogLevel};
pub use metrics::{CounterRegistry, MetricsHook, NoopMetrics};
