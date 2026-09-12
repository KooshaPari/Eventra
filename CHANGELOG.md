# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2026-08-30

### Added
- Prometheus-style metrics module (`infrastructure::metrics`) with atomic
  counters for `total_events_processed`, `total_handlers_registered`,
  `total_errors`, and `uptime_seconds`.
- `Metrics::render()` emitting the Prometheus text exposition format
  (`# HELP`, `# TYPE`, counter lines), plus `render_default_metrics()`.
- Minimal, dependency-free blocking `serve(addr, metrics)` HTTP server exposing
  `GET /metrics`.
- Unit tests for counter increments, render output format, and the metrics
  HTTP endpoint.

## [Unreleased]

