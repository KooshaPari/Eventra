//! Health and readiness probe types for services and CLI healthchecks.

use serde::{Deserialize, Serialize};

/// Overall health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Process is alive.
    Healthy,
    /// Process is alive but not ready to serve traffic.
    Degraded,
    /// Process should be restarted.
    Unhealthy,
}

/// Result of a single dependency probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Probe {
    /// Probe name (e.g. `event_store`, `outbox_relay`).
    pub name: String,
    /// Whether the probe passed.
    pub ok: bool,
    /// Optional detail message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// `/health` response — liveness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Top-level status.
    pub status: HealthStatus,
    /// Crate / service version.
    pub version: String,
    /// Individual probe results.
    pub probes: Vec<Probe>,
}

impl HealthReport {
    /// Minimal healthy report for library-only deployments.
    pub fn alive(version: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Healthy,
            version: version.into(),
            probes: vec![Probe {
                name: "process".into(),
                ok: true,
                detail: Some("eventkit library process alive".into()),
            }],
        }
    }
}

/// `/ready` response — readiness to accept work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessReport {
    /// Whether all readiness probes passed.
    pub ready: bool,
    /// Probe details.
    pub probes: Vec<Probe>,
}

impl ReadinessReport {
    /// Ready when every probe is `ok`.
    pub fn from_probes(probes: Vec<Probe>) -> Self {
        let ready = probes.iter().all(|p| p.ok);
        Self { ready, probes }
    }
}
