//! Structured logging initialization with configurable format and levels.

use std::sync::Once;
use tracing_subscriber::{fmt, EnvFilter, Layer};

static LOGGING_INIT: Once = Once::new();

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFormat {
    /// Human-readable plain text (default).
    #[default]
    Plain,
    /// JSON lines for log aggregators (Loki, CloudWatch, Datadog, etc.).
    Json,
}

/// Parsed log level filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLevel(pub String);

impl LogLevel {
    /// Default filter: `info` globally, `debug` for eventkit crates.
    pub fn default_filter() -> Self {
        Self("info,eventkit=debug,eventkit_obs=debug,phenotype_event_bus=debug".to_string())
    }

    /// Parse from `RUST_LOG` or fall back to [`Self::default_filter`].
    pub fn from_env_or_default() -> Self {
        std::env::var("RUST_LOG")
            .map(Self)
            .unwrap_or_else(|_| Self::default_filter())
    }
}

/// Initialize the global `tracing` subscriber (idempotent).
///
/// Honors `RUST_LOG` for filter directives and `EVENTKIT_LOG_FORMAT` for
/// output format (`plain` or `json`, default `plain`).
pub fn init_logging(level: Option<LogLevel>, format: Option<LogFormat>) {
    LOGGING_INIT.call_once(|| {
        let filter = level
            .map(|l| l.0)
            .unwrap_or_else(|| LogLevel::from_env_or_default().0);

        let env_filter = EnvFilter::try_new(&filter)
            .unwrap_or_else(|_| EnvFilter::new(LogLevel::default_filter().0));

        let fmt_choice = format.unwrap_or_else(|| {
            match std::env::var("EVENTKIT_LOG_FORMAT")
                .unwrap_or_default()
                .to_lowercase()
                .as_str()
            {
                "json" => LogFormat::Json,
                _ => LogFormat::Plain,
            }
        });

        match fmt_choice {
            LogFormat::Plain => {
                let layer = fmt::layer()
                    .with_target(true)
                    .with_level(true)
                    .with_thread_ids(false)
                    .with_filter(env_filter);
                let _ = tracing_subscriber::registry().with(layer).try_init();
            }
            LogFormat::Json => {
                let layer = fmt::layer()
                    .json()
                    .with_target(true)
                    .with_level(true)
                    .with_current_span(true)
                    .with_span_list(true)
                    .with_filter(env_filter);
                let _ = tracing_subscriber::registry().with(layer).try_init();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_filter_contains_eventkit() {
        let level = LogLevel::default_filter();
        assert!(level.0.contains("eventkit"));
    }
}
