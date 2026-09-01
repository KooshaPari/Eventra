# Deploying Eventra

This guide covers the runtime pieces that are actually shipped in this workspace: the `eventkit` framework crate, the `eventkit-obs` healthcheck binary, and the `phenotype-event-bus` outbox relay support.

## Prerequisites

- Rust 1.75+ from `rust-toolchain.toml`
- Docker 24+ if you want the container image or Compose stack
- A local copy of `.env.example` if you want a filled-out environment file

## Build and run locally

The workspace is library-first, so the normal way to work with it is to build the workspace and run the healthcheck binary from `eventkit-obs`.

```bash
cargo build --workspace
cargo run -p eventkit-obs --bin eventkit-healthcheck
```

To probe a running HTTP endpoint instead of using the library-only mode:

```bash
cargo run -p eventkit-obs --bin eventkit-healthcheck -- http://127.0.0.1:8080/health
```

## Docker

The repo includes a multi-stage Dockerfile and a Compose service for the healthcheck container.

```bash
docker build -t eventra/eventkit:latest .
docker run --rm eventra/eventkit:latest
docker compose up --build
```

The `docker-compose.yml` service exposes port `8080`, sets the tracing and health-check environment variables, and uses `eventkit-healthcheck` as the container healthcheck command.

## Health and readiness

`eventkit-healthcheck` supports two modes:

- No argument: library-only liveness check, exits `0` when the binary starts and the probe completes.
- URL argument: HTTP probe mode against `/health` or `/ready`.

The `eventkit-obs` crate also ships an optional HTTP health server behind the `http-health` feature.

Recommended orchestrator settings:

- `terminationGracePeriodSeconds: 30`
- send `SIGTERM` first, then `SIGKILL` only if shutdown exceeds the grace period

## Outbox relay runtime

The transactional outbox relay in `phenotype-event-bus` is the long-lived worker you need to plan for during deployment.

### Data flow

1. A command handler writes the domain mutation and an `OutboxEntry` in the same database transaction.
2. `OutboxRelay` claims unpublished rows from an `OutboxStore`.
3. The relay hands each row to a user-provided publisher closure or trait implementation.
4. On success, the row is marked published.
5. On failure, the row records the error and attempt count, then backs off before retrying.

### Storage backends

- `InMemoryOutbox` is for tests and single-process development only.
- `SqliteOutbox` is the embedded/dev adapter and works best for single-writer scenarios.
- `PostgresOutbox` is the durable multi-worker adapter and uses `SELECT ... FOR UPDATE SKIP LOCKED` to prevent duplicate claims.

### Operational rules

- Use one database transaction for the domain mutation and the outbox insert.
- Treat `OutboxEntry::id` as the consumer deduplication key.
- Keep publishers idempotent.
- Size relay shutdown time so it can finish the current batch and flush in-flight rows.

## Configuration

See [`.env.example`](../.env.example) for the current environment variables.

Relevant settings include:

- `RUST_LOG`
- `EVENTKIT_LOG_FORMAT`
- `EVENTKIT_HEALTH_PORT`
- `EVENTKIT_HEALTHCHECK_TIMEOUT_MS`
- `OUTBOX_POLL_INTERVAL_MS`
- `OUTBOX_BATCH_SIZE`
- `OUTBOX_SHUTDOWN_TIMEOUT_SECS`

## Related docs

- [`README.md`](../README.md)
- [`docs/disposition/phenotype-event-bus-runtime-boundary.md`](disposition/phenotype-event-bus-runtime-boundary.md)
- [`docs/remediation/OPS.md`](remediation/OPS.md)
- [`docs/remediation/OBSERVABILITY.md`](remediation/OBSERVABILITY.md)
