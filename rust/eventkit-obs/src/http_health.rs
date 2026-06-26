//! Optional Axum `/health` and `/ready` HTTP endpoints.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{routing::get, Json, Router};
use tokio::signal;
use tracing::info;

use crate::health::{HealthReport, ReadinessReport};

/// Shared state for health HTTP handlers.
#[derive(Clone)]
pub struct HealthState {
    /// Static version string reported in `/health`.
    pub version: String,
    /// Optional readiness probe callback.
    pub readiness: Arc<dyn Fn() -> ReadinessReport + Send + Sync>,
}

impl HealthState {
    /// Create with a static readiness report.
    pub fn always_ready(version: impl Into<String>) -> Self {
        let version = version.into();
        Self {
            version: version.clone(),
            readiness: Arc::new(move || {
                ReadinessReport::from_probes(vec![crate::health::Probe {
                    name: "default".into(),
                    ok: true,
                    detail: Some("no custom readiness probe configured".into()),
                }])
            }),
        }
    }
}

/// Build the health router (`GET /health`, `GET /ready`).
pub fn router(state: HealthState) -> Router {
    Router::new()
        .route(
            "/health",
            get({
                let state = state.clone();
                move || {
                    let state = state.clone();
                    async move {
                        let report = HealthReport::alive(state.version.clone());
                        Json(report)
                    }
                }
            }),
        )
        .route(
            "/ready",
            get({
                let state = state.clone();
                move || {
                    let state = state.clone();
                    async move {
                        let report = (state.readiness)();
                        Json(report)
                    }
                }
            }),
        )
}

/// Bind and serve the health router until SIGINT/SIGTERM (graceful shutdown).
pub async fn serve(addr: SocketAddr, state: HealthState) -> Result<(), std::io::Error> {
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "eventkit health server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("received Ctrl+C, shutting down health server"),
        _ = terminate => info!("received SIGTERM, shutting down health server"),
    }
}
