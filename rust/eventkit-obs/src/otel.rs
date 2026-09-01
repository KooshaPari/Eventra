//! OpenTelemetry initialization helpers.
//!
//! This module is strictly feature-gated behind `otel`.

#![warn(missing_docs)]

use opentelemetry::global;
use opentelemetry_sdk::runtime::Tokio;
use opentelemetry_sdk::trace::{self, TracerProvider};
use opentelemetry_sdk::Resource;
use tracing_subscriber::Registry;

/// Options for OTLP initialization.
#[derive(Debug, Clone)]
pub struct OtlpConfig {
    /// Full OTLP endpoint URL (for example `http://127.0.0.1:4317`).
    pub endpoint: String,
    /// Service name attached to all emitted spans.
    pub service_name: String,
    /// Optional service version reported in resource attributes.
    pub service_version: Option<String>,
    /// Export timeout in seconds.
    pub export_timeout_secs: u64,
}

impl Default for OtlpConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:4317".to_string(),
            service_name: "eventra".to_string(),
            service_version: None,
            export_timeout_secs: 2,
        }
    }
}

/// Error from OTEL initialization.
#[derive(Debug, thiserror::Error)]
pub enum OtlpError {
    /// Layer/tracer construction failed.
    #[error("otel initialization failed: {0}")]
    Initialize(String),
}

/// RAII handle returned by [`install_otel`].
#[derive(Debug)]
pub struct OtlpHandle {
    provider: TracerProvider,
}

impl Drop for OtlpHandle {
    fn drop(&mut self) {
        global::shutdown_tracer_provider();
    }
}

/// Prepared OTEL layer + provider tuple from [`install_otel`].
pub struct OtlpLayer {
    pub(crate) layer: tracing_opentelemetry::OpenTelemetryLayer<
        Registry,
        opentelemetry_sdk::trace::Tracer,
    >,
    pub(crate) handle: OtlpHandle,
}

impl OtlpLayer {
    /// Consume this wrapper and return a tracing layer you can attach anywhere.
    pub fn into_layer(self) -> tracing_opentelemetry::OpenTelemetryLayer<
        Registry,
        opentelemetry_sdk::trace::Tracer,
    > {
        self.layer
    }

    /// Keep this handle alive as long as OTEL is expected to remain active.
    pub fn keep_alive(&self) -> &OtlpHandle {
        &self.handle
    }
}

/// Build and register a provider for OTLP export, returning a tracing layer.
pub fn install_otel(config: &OtlpConfig) -> Result<OtlpLayer, OtlpError> {
    std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", &config.endpoint);
    std::env::set_var(
        "OTEL_EXPORTER_OTLP_TIMEOUT",
        format!("{}s", config.export_timeout_secs),
    );

    let mut attrs = Vec::new();
    attrs.push(opentelemetry::KeyValue::new("service.name", config.service_name.clone()));
    if let Some(version) = &config.service_version {
        attrs.push(opentelemetry::KeyValue::new("service.version", version.clone()));
    }

    let provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_trace_config(trace::Config::default().with_resource(Resource::new(attrs)))
        .install_batch(Tokio)
        .map_err(|e| OtlpError::Initialize(e.to_string()))?;

    let tracer = provider.tracer(&config.service_name);
    global::set_tracer_provider(provider.clone());
    let layer = tracing_opentelemetry::layer().with_tracer(tracer);

    Ok(OtlpLayer {
        layer,
        handle: OtlpHandle { provider },
    })
}

/// Build a correlation-aware span for event bus operations.
pub fn with_correlation_span<T, F>(correlation_id: Option<&str>, operation: &str, f: F) -> T
where
    F: FnOnce() -> T,
{
    let span = match correlation_id {
        Some(cid) => tracing::info_span!(
            operation,
            correlation_id = %cid,
            eventbus = "span"
        ),
        None => tracing::info_span!(operation),
    };
    let _guard = span.enter();
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otlp_config_defaults_are_set() {
        let cfg = OtlpConfig::default();
        assert_eq!(cfg.service_name, "eventra");
    }

    #[test]
    fn span_helper_executes() {
        with_correlation_span(Some("corr-1"), "eventbus.test", || {
            tracing::info!("span works");
        });
    }
}
