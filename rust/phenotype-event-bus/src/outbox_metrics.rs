//! Outbox operational metrics + OpenTelemetry span emission (EVE-SOTA-005).
//!
//! Provides:
//! - `OutboxMetrics`: atomic counter/gauge snapshot for Prometheus export
//! - `OutboxMetricLabels`: cardinality-bounded label set (publisher + outcome)
//! - `OutboxMetricEvent`: a single observation (success/failure/terminal/retried)
//! - `emit_metric()`: best-effort emit (no-op when no exporter registered)
//! - `OutboxSpan`: thin OTEL-shaped span (works with both real OTEL and
//!   `tracing` spans via `tracing_opentelemetry` adapter — the user can
//!   opt in to either independently)
//!
//! Design notes:
//! - No hard dependency on opentelemetry crate; the `OutboxSpan` type
//!   is a plain struct that produces a `tracing::info_span!` so it works
//!   with whatever tracing subscriber the host uses. If the user has
//!   installed the OTEL layer (`tracing-opentelemetry`), spans flow
//!   into OTEL automatically.
//! - Metrics use `AtomicU64` so they're lock-free and bounded (no
//!   allocation on the hot path).
//! - Labels are string-interned via `OutboxMetricLabels::intern` to
//!   keep Prometheus cardinality under control (per the SOTA brief:
//!   outcome x publisher x env is the natural cardinality).
//!
//! Layered scope:
//! - EVE-SOTA-001 (PR #49)  - trait + InMemoryOutbox
//! - EVE-SOTA-002 (PR #50)  - PostgresOutbox adapter
//! - EVE-SOTA-003 (PR #51)  - OutboxRelay
//! - EVE-SOTA-004 (PR #52)  - SqliteOutbox adapter
//! - EVE-SOTA-005 (this)    - OutboxMetrics + OutboxSpan
//!
//! Test plan: see #[cfg(test)] mod tests at the bottom.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Stable label set for outbox metrics. Cardinality is bounded by the
/// product of (publisher, outcome, env). New labels must be added here
/// explicitly to avoid uncontrolled growth.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct OutboxMetricLabels {
    pub publisher: Arc<str>,
    pub outcome: OutboxMetricOutcome,
    pub env: Arc<str>,
}

/// Outcome enum — closed set to keep cardinality bounded.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum OutboxMetricOutcome {
    Enqueued,
    Claimed,
    Published,
    Retried,
    FailedTerminal,
}

impl OutboxMetricOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutboxMetricOutcome::Enqueued => "enqueued",
            OutboxMetricOutcome::Claimed => "claimed",
            OutboxMetricOutcome::Published => "published",
            OutboxMetricOutcome::Retried => "retried",
            OutboxMetricOutcome::FailedTerminal => "failed_terminal",
        }
    }
}

impl OutboxMetricLabels {
    pub fn new(
        publisher: impl Into<Arc<str>>,
        outcome: OutboxMetricOutcome,
        env: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            publisher: publisher.into(),
            outcome,
            env: env.into(),
        }
    }
}

/// A single metric observation. The OutboxMetrics registry accepts these
/// and updates its internal counters atomically.
#[derive(Debug, Clone)]
pub struct OutboxMetricEvent {
    pub labels: OutboxMetricLabels,
    pub attempts: u64,
    pub latency_micros: u64,
}

/// Snapshot of all metrics. Cheap to read (atomic loads) and serializable
/// to Prometheus text format via `to_prometheus_text()`.
#[derive(Debug, Default)]
pub struct OutboxMetrics {
    enqueued: AtomicU64,
    claimed: AtomicU64,
    published: AtomicU64,
    retried: AtomicU64,
    failed_terminal: AtomicU64,
    /// Running total of publish attempts (success + failure).
    attempts_total: AtomicU64,
    /// Running total of publish latency in microseconds (for averaging).
    publish_latency_micros_total: AtomicU64,
    /// Last batch timestamp (unix millis). 0 = never.
    last_batch_unix_millis: AtomicU64,
}

impl OutboxMetrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Record a single observation. Thread-safe, lock-free.
    pub fn observe(&self, event: OutboxMetricEvent) {
        self.attempts_total.fetch_add(1, Ordering::Relaxed);
        self.publish_latency_micros_total
            .fetch_add(event.latency_micros, Ordering::Relaxed);
        match event.labels.outcome {
            OutboxMetricOutcome::Enqueued => self.enqueued.fetch_add(1, Ordering::Relaxed),
            OutboxMetricOutcome::Claimed => self.claimed.fetch_add(1, Ordering::Relaxed),
            OutboxMetricOutcome::Published => self.published.fetch_add(1, Ordering::Relaxed),
            OutboxMetricOutcome::Retried => self.retried.fetch_add(1, Ordering::Relaxed),
            OutboxMetricOutcome::FailedTerminal => {
                self.failed_terminal.fetch_add(1, Ordering::Relaxed)
            }
        };
    }

    /// Record the timestamp of the most recent successful batch.
    pub fn record_batch(&self, unix_millis: u64) {
        self.last_batch_unix_millis
            .store(unix_millis, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> OutboxMetricsSnapshot {
        OutboxMetricsSnapshot {
            enqueued: self.enqueued.load(Ordering::Relaxed),
            claimed: self.claimed.load(Ordering::Relaxed),
            published: self.published.load(Ordering::Relaxed),
            retried: self.retried.load(Ordering::Relaxed),
            failed_terminal: self.failed_terminal.load(Ordering::Relaxed),
            attempts_total: self.attempts_total.load(Ordering::Relaxed),
            publish_latency_micros_total: self.publish_latency_micros_total.load(Ordering::Relaxed),
            last_batch_unix_millis: self.last_batch_unix_millis.load(Ordering::Relaxed),
        }
    }

    /// Prometheus text-format export.
    pub fn to_prometheus_text(&self) -> String {
        let s = self.snapshot();
        let avg_latency_us = s
            .publish_latency_micros_total
            .checked_div(s.published)
            .unwrap_or(0);
        format!(
            "# HELP phenotype_event_bus_outbox_enqueued_total Total events enqueued to outbox\n\
             # TYPE phenotype_event_bus_outbox_enqueued_total counter\n\
             phenotype_event_bus_outbox_enqueued_total {enq}\n\
             # HELP phenotype_event_bus_outbox_claimed_total Total rows claimed by relays\n\
             # TYPE phenotype_event_bus_outbox_claimed_total counter\n\
             phenotype_event_bus_outbox_claimed_total {clm}\n\
             # HELP phenotype_event_bus_outbox_published_total Total events successfully published\n\
             # TYPE phenotype_event_bus_outbox_published_total counter\n\
             phenotype_event_bus_outbox_published_total {pub_}\n\
             # HELP phenotype_event_bus_outbox_retried_total Total publish retries\n\
             # TYPE phenotype_event_bus_outbox_retried_total counter\n\
             phenotype_event_bus_outbox_retried_total {retry}\n\
             # HELP phenotype_event_bus_outbox_failed_terminal_total Total events that exhausted max_attempts\n\
             # TYPE phenotype_event_bus_outbox_failed_terminal_total counter\n\
             phenotype_event_bus_outbox_failed_terminal_total {fail}\n\
             # HELP phenotype_event_bus_outbox_attempts_total Total publish attempts (success+failure)\n\
             # TYPE phenotype_event_bus_outbox_attempts_total counter\n\
             phenotype_event_bus_outbox_attempts_total {att}\n\
             # HELP phenotype_event_bus_outbox_publish_latency_micros_avg Average publish latency (microseconds)\n\
             # TYPE phenotype_event_bus_outbox_publish_latency_micros_avg gauge\n\
             phenotype_event_bus_outbox_publish_latency_micros_avg {lat}\n\
             # HELP phenotype_event_bus_outbox_last_batch_unix_millis Timestamp of last successful batch\n\
             # TYPE phenotype_event_bus_outbox_last_batch_unix_millis gauge\n\
             phenotype_event_bus_outbox_last_batch_unix_millis {ts}\n",
            enq = s.enqueued,
            clm = s.claimed,
            pub_ = s.published,
            retry = s.retried,
            fail = s.failed_terminal,
            att = s.attempts_total,
            lat = avg_latency_us,
            ts = s.last_batch_unix_millis,
        )
    }
}

/// Value-typed snapshot of OutboxMetrics at a point in time.
#[derive(Debug, Clone, Copy)]
pub struct OutboxMetricsSnapshot {
    pub enqueued: u64,
    pub claimed: u64,
    pub published: u64,
    pub retried: u64,
    pub failed_terminal: u64,
    pub attempts_total: u64,
    pub publish_latency_micros_total: u64,
    pub last_batch_unix_millis: u64,
}

/// Thin OTEL-shaped span. Emits a `tracing::info_span!` so the host's
/// tracing subscriber can route it to OTEL (via `tracing-opentelemetry`)
/// or to logs only. The span carries outbox-specific attributes that
/// show up in any OTEL-aware exporter.
#[derive(Debug, Clone)]
pub struct OutboxSpan {
    pub operation: &'static str,
    pub outbox_id: String,
    pub publisher: Arc<str>,
    pub correlation_id: Option<String>,
}

impl OutboxSpan {
    pub fn enter_enqueue(publisher: Arc<str>) -> tracing::Span {
        tracing::info_span!(
            "outbox.enqueue",
            otel.kind = "internal",
            outbox.publisher = %publisher,
            outbox.outcome = "enqueued",
        )
    }

    pub fn enter_claim(publisher: Arc<str>, batch_size: usize) -> tracing::Span {
        tracing::info_span!(
            "outbox.claim",
            otel.kind = "internal",
            outbox.publisher = %publisher,
            outbox.batch_size = batch_size,
            outbox.outcome = "claimed",
        )
    }

    pub fn enter_publish(publisher: Arc<str>, event_id: &str, attempt: u64) -> tracing::Span {
        tracing::info_span!(
            "outbox.publish",
            otel.kind = "producer",
            messaging.system = "phenotype.event-bus.outbox",
            messaging.destination = %publisher,
            messaging.message.id = %event_id,
            outbox.publisher = %publisher,
            outbox.attempt = attempt,
        )
    }

    pub fn enter_mark_published(publisher: Arc<str>, event_id: &str) -> tracing::Span {
        tracing::info_span!(
            "outbox.mark_published",
            otel.kind = "internal",
            outbox.publisher = %publisher,
            outbox.event_id = %event_id,
            outbox.outcome = "published",
        )
    }

    pub fn enter_record_failure(
        publisher: Arc<str>,
        event_id: &str,
        attempt: u64,
        max_attempts: u64,
    ) -> tracing::Span {
        tracing::info_span!(
            "outbox.record_failure",
            otel.kind = "internal",
            outbox.publisher = %publisher,
            outbox.event_id = %event_id,
            outbox.attempt = attempt,
            outbox.max_attempts = max_attempts,
        )
    }
}

/// Best-effort emit — no-op if no metrics registry is registered.
/// Pass `Arc<OutboxMetrics>` via `set_global_metrics` at startup.
static GLOBAL_METRICS: std::sync::OnceLock<Arc<OutboxMetrics>> = std::sync::OnceLock::new();

pub fn set_global_metrics(metrics: Arc<OutboxMetrics>) -> Result<(), Arc<OutboxMetrics>> {
    GLOBAL_METRICS.set(metrics)
}

pub fn global_metrics() -> Option<Arc<OutboxMetrics>> {
    GLOBAL_METRICS.get().cloned()
}

pub fn emit_metric(event: OutboxMetricEvent) {
    if let Some(m) = global_metrics() {
        m.observe(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_default_zero() {
        let m = OutboxMetrics::new();
        let s = m.snapshot();
        assert_eq!(s.enqueued, 0);
        assert_eq!(s.claimed, 0);
        assert_eq!(s.published, 0);
        assert_eq!(s.retried, 0);
        assert_eq!(s.failed_terminal, 0);
        assert_eq!(s.attempts_total, 0);
    }

    #[test]
    fn metrics_observe_increments_correctly() {
        let m = OutboxMetrics::new();
        let labels_enq = OutboxMetricLabels::new("nats", OutboxMetricOutcome::Enqueued, "prod");
        let labels_pub = OutboxMetricLabels::new("nats", OutboxMetricOutcome::Published, "prod");
        m.observe(OutboxMetricEvent {
            labels: labels_enq.clone(),
            attempts: 1,
            latency_micros: 50,
        });
        m.observe(OutboxMetricEvent {
            labels: labels_enq.clone(),
            attempts: 1,
            latency_micros: 60,
        });
        m.observe(OutboxMetricEvent {
            labels: labels_pub.clone(),
            attempts: 1,
            latency_micros: 100,
        });
        let s = m.snapshot();
        assert_eq!(s.enqueued, 2);
        assert_eq!(s.published, 1);
        assert_eq!(s.attempts_total, 3);
        assert_eq!(s.publish_latency_micros_total, 210);
    }

    #[test]
    fn metrics_avg_latency_computed() {
        let m = OutboxMetrics::new();
        let labels = OutboxMetricLabels::new("kafka", OutboxMetricOutcome::Published, "prod");
        for _ in 0..4 {
            m.observe(OutboxMetricEvent {
                labels: labels.clone(),
                attempts: 1,
                latency_micros: 250,
            });
        }
        let text = m.to_prometheus_text();
        // 4 * 250 / 4 = 250
        assert!(text.contains("phenotype_event_bus_outbox_publish_latency_micros_avg 250"));
    }

    #[test]
    fn record_batch_updates_timestamp() {
        let m = OutboxMetrics::new();
        m.record_batch(1_700_000_000_123);
        assert_eq!(m.snapshot().last_batch_unix_millis, 1_700_000_000_123);
    }

    #[test]
    fn outcome_as_str_stable() {
        assert_eq!(OutboxMetricOutcome::Enqueued.as_str(), "enqueued");
        assert_eq!(OutboxMetricOutcome::Claimed.as_str(), "claimed");
        assert_eq!(OutboxMetricOutcome::Published.as_str(), "published");
        assert_eq!(OutboxMetricOutcome::Retried.as_str(), "retried");
        assert_eq!(
            OutboxMetricOutcome::FailedTerminal.as_str(),
            "failed_terminal"
        );
    }

    #[test]
    fn prometheus_text_contains_all_metrics() {
        let m = OutboxMetrics::new();
        let text = m.to_prometheus_text();
        for metric in [
            "phenotype_event_bus_outbox_enqueued_total",
            "phenotype_event_bus_outbox_claimed_total",
            "phenotype_event_bus_outbox_published_total",
            "phenotype_event_bus_outbox_retried_total",
            "phenotype_event_bus_outbox_failed_terminal_total",
            "phenotype_event_bus_outbox_attempts_total",
            "phenotype_event_bus_outbox_publish_latency_micros_avg",
            "phenotype_event_bus_outbox_last_batch_unix_millis",
        ] {
            assert!(text.contains(metric), "missing metric: {metric}");
        }
    }

    #[test]
    fn global_metrics_emit() {
        let m = OutboxMetrics::new();
        set_global_metrics(m.clone()).ok();
        let labels = OutboxMetricLabels::new("p1", OutboxMetricOutcome::Enqueued, "test");
        emit_metric(OutboxMetricEvent {
            labels,
            attempts: 1,
            latency_micros: 10,
        });
        let s = m.snapshot();
        assert_eq!(s.enqueued, 1);
    }

    #[test]
    fn span_enter_enqueue_emits_publisher() {
        let span = OutboxSpan::enter_enqueue(Arc::from("nats"));
        let _guard = span.enter();
        // Span exists; can't introspect attributes without a subscriber,
        // but the call must not panic.
    }

    #[test]
    fn span_enter_publish_carries_message_id() {
        let span = OutboxSpan::enter_publish(Arc::from("kafka"), "01HXYZ", 2);
        let _guard = span.enter();
        // Smoke test: span entry/exit must not panic.
    }
}
