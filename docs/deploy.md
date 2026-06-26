# Deploying Eventra / eventkit

This guide covers container deployment, health probes, configuration, and graceful shutdown for services built on the Eventra Rust workspace.

## Prerequisites

- Rust 1.75+ (see `rust-toolchain.toml`)
- Docker 24+ (for container deploy)
- Copy `.env.example` to `.env` and adjust non-secret settings

## Quick local smoke

```bash
# Build healthcheck CLI from the observability crate
cd rust/eventkit-obs
cargo build --release

# Liveness (library-only mode — exits 0, prints JSON health report)
./target/release/eventkit-healthcheck
```

## Docker (multi-stage)

```bash
docker build -t eventra/eventkit:latest .
docker run --rm eventra/eventkit:latest
```

Or with Compose:

```bash
docker compose up --build
```

The image ships `eventkit-healthcheck` as the entrypoint. Override `CMD` to probe an HTTP endpoint:

```bash
docker run --rm eventra/eventkit:latest http://127.0.0.1:8080/health
```

## Health and readiness

| Endpoint / command | Purpose | When to use |
|---|---|---|
| `eventkit-healthcheck` (no args) | Liveness — process alive | Library-only, sidecars |
| `GET /health` | Liveness — HTTP JSON report | Service binaries with `http-health` feature |
| `GET /ready` | Readiness — dependencies OK | Before routing traffic |
| `eventkit-healthcheck http://host:8080/ready` | CLI readiness probe | K8s `exec` probes, CI |

Example Kubernetes probes:

```yaml
livenessProbe:
  exec:
    command: ["eventkit-healthcheck"]
  initialDelaySeconds: 10
  periodSeconds: 30
readinessProbe:
  httpGet:
    path: /ready
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 10
```

## Configuration

All settings are environment variables — see [`.env.example`](../.env.example). No secrets are committed.

| Variable | Default | Description |
|---|---|---|
| `RUST_LOG` | `info,eventkit=debug,...` | tracing filter |
| `EVENTKIT_LOG_FORMAT` | `plain` | `plain` or `json` |
| `EVENTKIT_HEALTH_PORT` | `8080` | HTTP health server bind port |
| `EVENTKIT_HEALTHCHECK_TIMEOUT_MS` | `3000` | CLI HTTP probe timeout |

## Graceful shutdown

Eventra components that run long-lived workers (e.g. `phenotype-event-bus` outbox relay) should:

1. **Trap SIGTERM and SIGINT** — Tokio services use `tokio::signal` (see `rust/eventkit-obs/src/http_health.rs`).
2. **Stop accepting new work** — close subscriptions / HTTP listeners first.
3. **Drain in-flight events** — allow the outbox relay `Shutdown` token to flush pending publishes.
4. **Bound wait time** — use `OUTBOX_SHUTDOWN_TIMEOUT_SECS` (default 30) then exit.

Container orchestrators:

- Set `terminationGracePeriodSeconds` ≥ 30 (matches `docker-compose.yml` `stop_grace_period`).
- Send SIGTERM before SIGKILL; the Dockerfile sets `STOPSIGNAL SIGTERM`.

## CI / release

GitHub Actions runs `cargo test --all` on push (`.github/workflows/ci.yml`). For production images, pin the Rust toolchain digest and enable SLSA attestation (see `docs/slsa.md`).

## Further integration

Wire the `eventkit-obs` crate into the workspace and service binaries — exact diffs in:

- [`docs/remediation/OBSERVABILITY.md`](remediation/OBSERVABILITY.md)
- [`docs/remediation/OPS.md`](remediation/OPS.md)
