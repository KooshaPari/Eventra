# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.1.0] - 2026-08-28

### Added

- **CQRS / Event Sourcing building blocks** — `phenotype-event-sourcing` crate
  with domain entities, ports (inbound/outbound), adapters, and application
  use-case layer following hexagonal architecture.
- **Hash chain integrity** — SHA-256 hash chain computation and verification
  (`compute_hash`, `verify_chain`, `detect_gaps`) ensuring tamper-evident
  event sequences.
- **Transactional outbox pattern** — `phenotype-event-bus` outbox module
  (`OutboxStore` trait, `InMemoryOutbox`, Postgres and SQLite adapters)
  solving the dual-write problem with at-least-once relay semantics.
- **Prometheus-compatible metrics** — `MetricsHook` trait, `CounterRegistry`,
  and `NoopMetrics` in `eventkit-obs` for counters, histograms, and gauges
  without mandating a Prometheus exporter.
- **OpenTelemetry tracing** — `eventkit-obs` OTel module (`install_otel`,
  `OtlpConfig`, correlation-aware spans) for distributed tracing via OTLP.
- **Structured logging and correlation IDs** — `eventkit-obs` logging and
  correlation modules (`init_logging`, `correlation_id`, `set_correlation_id`).
- **Health / readiness probes** — `HealthReport`, `ReadinessReport`, and
  optional HTTP health endpoint (`http-health` feature) in `eventkit-obs`.
- **Event contracts** — `phenotype-event-contracts` with `EventEnvelope`,
  `EventBus` trait, `EventStore` trait, and error types.
