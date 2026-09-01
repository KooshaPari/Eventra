# Observability remediation (audit area G)

Additive observability hardening for Eventra. New code lives in `rust/eventkit-obs/`. Apply the diffs below to wire it into the workspace and existing crates.

## What was added

| Artifact | Purpose |
|---|---|
| `rust/eventkit-obs/` | Structured logging, correlation IDs, metrics hook, health types |
| `rust/eventkit-obs/src/bin/healthcheck.rs` | CLI liveness/readiness probe |
| `.env.example` | `RUST_LOG`, `EVENTKIT_LOG_FORMAT`, health timeouts |

## Gap analysis

| Requirement | Status | Location |
|---|---|---|
| Structured logging + log levels | **Added** (crate) | `eventkit-obs::logging` |
| `/health` + `/ready` | **Added** (optional HTTP) | `eventkit-obs::http_health` feature |
| CLI healthcheck | **Added** | `eventkit-healthcheck` binary |
| Request/trace correlation ID | **Added** | `eventkit-obs::correlation` |
| Metrics hook | **Added** | `eventkit-obs::metrics::MetricsHook` |
| Wire into `eventkit` main crate | **Pending** | diffs below |
| Prometheus export | **Future** | remediation note at bottom |

---

## Diff 1 — Add `eventkit-obs` to workspace

**File:** `Cargo.toml`

```diff
 members = [
     ".",
     "rust/phenotype-event-contracts",
     "rust/phenotype-event-bus",
     "rust/phenotype-event-sourcing",
     "rust/phenotype-error-core",
+    "rust/eventkit-obs",
 ]
```

```diff
 tracing = "0.1"
 tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
+eventkit-obs = { path = "rust/eventkit-obs" }
```

**File:** `Cargo.toml` (root package `[dependencies]`)

```diff
 tracing = { workspace = true }
 tracing-subscriber = { workspace = true }
+eventkit-obs = { workspace = true }
```

---

## Diff 2 — Re-export observability from `eventkit`

**File:** `src/lib.rs`

```diff
 pub use domain::*;

+/// Observability helpers (structured logging, correlation, metrics, health).
+pub mod observability {
+    pub use eventkit_obs::{
+        correlation_id, ensure_correlation_id, set_correlation_id, CORRELATION_FIELD,
+        init_logging, CounterRegistry, HealthReport, HealthStatus, LogFormat, LogLevel,
+        MetricsHook, NoopMetrics, Probe, ReadinessReport, SharedMetrics,
+    };
+}
+
 use std::sync::Once;
```

Optionally delegate `init_tracing` to the new helper:

```diff
 pub fn init_tracing() {
-    TRACING_INIT.call_once(|| {
-        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
-            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,eventkit=debug"));
-
-        let _ = tracing_subscriber::fmt()
-            .with_env_filter(env_filter)
-            .with_target(true)
-            .with_level(true)
-            .try_init();
-    });
+    TRACING_INIT.call_once(|| {
+        eventkit_obs::init_logging(None, None);
+    });
 }
```

---

## Diff 3 — Infrastructure module stub

**File:** `src/infrastructure/mod.rs`

```diff
 //! Infrastructure Layer
 //!
-//! Cross-cutting infrastructure concerns (logging, metrics, etc.) will be added
-//! here as the framework grows. Domain errors live in [`crate::domain::error`]
-//! and are not re-exported.
+//! Cross-cutting infrastructure concerns. Domain errors live in
+//! [`crate::domain::error`] and are not re-exported.
+
+pub mod observability {
+    pub use eventkit_obs::*;
+}
```

---

## Diff 4 — Correlation ID in event bus spans

**File:** `src/application/event_bus.rs`

```diff
 use tracing::{debug, info, instrument};
+use uuid::Uuid;
+
+use crate::observability::{correlation_id, ensure_correlation_id, CORRELATION_FIELD};
```

Inside `publish`, after entering the span:

```diff
     fn publish(&self, event: &Event) -> Result<(), EventError> {
+        let cid = event
+            .metadata
+            .correlation_id
+            .or_else(correlation_id)
+            .unwrap_or_else(ensure_correlation_id);
+        tracing::Span::current().record(CORRELATION_FIELD, tracing::field::display(cid));
+
         let event_type = &event.metadata.event_type;
```

---

## Diff 5 — Metrics hook on event bus (optional)

**File:** `src/application/event_bus.rs`

```diff
 pub struct InMemoryEventBus {
     subscribers: RwLock<HashMap<String, Vec<Arc<dyn EventHandler>>>>,
+    metrics: eventkit_obs::SharedMetrics,
 }

 impl InMemoryEventBus {
-    pub fn new() -> Self {
+    pub fn new() -> Self {
+        Self::with_metrics(Arc::new(eventkit_obs::NoopMetrics))
+    }
+
+    pub fn with_metrics(metrics: eventkit_obs::SharedMetrics) -> Self {
         Self {
             subscribers: RwLock::new(HashMap::new()),
+            metrics,
         }
     }
 }
```

After successful publish:

```diff
+                self.metrics.increment_counter(
+                    "eventkit_events_published_total",
+                    &[("event_type", event_type.as_str())],
+                    1,
+                );
                 Ok(())
```

---

## Diff 6 — HTTP health server in a service binary

Create `src/bin/eventkit-health.rs` (new file) and add to root `Cargo.toml`:

```toml
[[bin]]
name = "eventkit-health"
path = "src/bin/eventkit-health.rs"
required-features = ["http-health"]

[features]
http-health = ["eventkit-obs/http-health", "tokio"]
```

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    eventkit_obs::init_logging(None, None);
    let port: u16 = std::env::var("EVENTKIT_HEALTH_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    eventkit_obs::http_health::serve(addr, eventkit_obs::http_health::HealthState::always_ready(env!("CARGO_PKG_VERSION"))).await?;
    Ok(())
}
```

---

## Diff 7 — `phenotype-event-bus` relay tracing

**File:** `rust/phenotype-event-bus/Cargo.toml`

```diff
 tracing = { workspace = true }
+eventkit-obs = { workspace = true }
```

Instrument `outbox_relay.rs` publish loop with correlation from envelope metadata when present.

---

## Verification checklist

- [ ] `cargo test -p eventkit-obs`
- [ ] `RUST_LOG=debug cargo test -p eventkit` — spans visible
- [ ] `eventkit-healthcheck` exits 0
- [ ] `EVENTKIT_LOG_FORMAT=json` emits JSON lines
- [ ] `curl localhost:8080/health` returns `{"status":"healthy",...}` (with http-health binary)

## Future: Prometheus

Export `CounterRegistry::snapshot()` at `GET /metrics` or integrate `metrics` + `metrics-exporter-prometheus` crates behind a feature flag. Keep the `MetricsHook` trait as the stable injection point.
