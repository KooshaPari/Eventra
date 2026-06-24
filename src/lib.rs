//! # eventkit - Event-Driven Architecture Framework
//!
//! CQRS and Event Sourcing with hexagonal architecture.
//!
//! ## Observability
//!
//! This crate ships with a [`tracing`](https://docs.rs/tracing) integration.
//! The EventBus, command handler, and event store emit `info`-level spans
//! and events for publish, subscribe, command handling, and append/read
//! operations. To see those events, call [`init_tracing`] once during
//! application startup:
//!
//! ```no_run
//! eventkit::init_tracing();
//! ```
//!
//! The default configuration honors the `RUST_LOG` environment variable
//! (falling back to `info,eventkit=debug`) and writes structured logs to
//! stderr. Calling `init_tracing` more than once is a no-op.

pub mod adapters;
pub mod application;
pub mod domain;
pub mod infrastructure;

pub use adapters::*;
pub use application::*;
pub use domain::*;

use std::sync::Once;

static TRACING_INIT: Once = Once::new();

/// Initialize the global `tracing` subscriber for the `eventkit` crate.
///
/// Idempotent — safe to call multiple times; only the first call has an
/// effect. Reads the `RUST_LOG` environment variable to configure the
/// default filter (falling back to `info,eventkit=debug` when unset).
///
/// For full control over the subscriber (e.g. OTLP export, JSON output,
/// custom layers), build your own `tracing_subscriber::Registry` and skip
/// this helper.
pub fn init_tracing() {
    TRACING_INIT.call_once(|| {
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,eventkit=debug"));

        let _ = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_level(true)
            .try_init();
    });
}
