# Eventra

Eventra is a Rust workspace for event-driven systems with CQRS/event-sourcing building blocks, a framework-local in-memory event bus, and transactional outbox support.

## Workspace layout

- `eventkit` in `src/` is the top-level framework crate.
- `eventkit-obs` in `rust/eventkit-obs/` provides structured logging, health probes, and the `eventkit-healthcheck` CLI.
- `phenotype-event-contracts` defines event and event-bus traits.
- `phenotype-event-bus` provides the generic event envelope, in-memory bus, outbox traits, relay, metrics, and SQLite/Postgres adapters behind features.
- `phenotype-event-sourcing` and `phenotype-error-core` provide supporting framework types.

## Install

Prerequisites:

- Rust 1.75+ from `rust-toolchain.toml`

Clone and build the workspace:

```bash
git clone https://github.com/KooshaPari/Eventra.git
cd Eventra
cargo build --workspace
```

To use the crates from another project, add the crate you need to your `Cargo.toml` and depend on the workspace package directly, for example:

```toml
[dependencies]
eventkit = { git = "https://github.com/KooshaPari/Eventra", package = "eventkit" }
phenotype-event-bus = { git = "https://github.com/KooshaPari/Eventra", package = "phenotype-event-bus" }
```

## Run

The repository is library-first. The only shipped binary is the healthcheck CLI in `eventkit-obs`.

```bash
cargo run -p eventkit-obs --bin eventkit-healthcheck
```

For HTTP probe mode, point the CLI at a health endpoint exposed by your service:

```bash
cargo run -p eventkit-obs --bin eventkit-healthcheck -- http://127.0.0.1:8080/health
```

Docker and Compose are available for the local healthcheck container:

```bash
docker build -t eventra/eventkit:latest .
docker compose up --build
```

## Usage

### Framework crate

`eventkit` exposes the framework modules and an idempotent tracing initializer:

```rust
eventkit::init_tracing();
```

The default tracing filter honors `RUST_LOG` and falls back to `info,eventkit=debug`.

### In-memory event bus

`src/application/event_bus.rs` implements a synchronous in-process bus over the framework `Event` and `EventHandler` traits.

- `publish` routes by `event.metadata.event_type`.
- `subscribe` registers one handler per event type returned by `handler.event_types()`.
- A failing handler is logged and does not abort delivery to the remaining handlers.

### Generic event bus and envelope

`phenotype-event-bus` defines:

- `EventId` as a ULID-backed identifier.
- `EventEnvelope<T>` with `source`, `timestamp`, `payload`, `correlation_id`, and `causation_id`.
- `EventBus` as an async publish/subscribe/request/close trait.
- `memory::InMemoryEventBus` for tests and simple local use.

Example:

```rust
use phenotype_event_bus::EventEnvelope;

let envelope = EventEnvelope::new("orders.created", serde_json::json!({"id": 1}))
    .with_correlation_id("cor-123")
    .with_causation_id("cause-456");
```

### Transactional outbox

`phenotype-event-bus` also provides the transactional outbox pattern:

- `OutboxEntry` stores the envelope, aggregate id, timestamps, attempt count, and last error.
- `OutboxStore` is the storage trait.
- `InMemoryOutbox` is for tests and single-process development.
- `PostgresOutbox` is enabled by the `postgres` feature and uses `SELECT ... FOR UPDATE SKIP LOCKED` to support multiple relay workers.
- `SqliteOutbox` is enabled by the `sqlite` feature and is intended for embedded/dev/test use.
- `OutboxRelay` drains unpublished rows, invokes a user-supplied publisher, records failures, and backs off between retries.

The key rule is dual-write safety: write the domain mutation and the outbox row in the same database transaction, then let the relay publish later. Consumers must treat `OutboxEntry::id` as the deduplication key.

## Architecture

```text
Framework crate (eventkit)
├─ domain
│  ├─ Event, Aggregate, EventHandler traits
│  └─ EventBus port
├─ application
│  └─ in-memory EventBus adapter
└─ infrastructure
   └─ supporting adapters and integration helpers

Runtime/event-bus support crate (phenotype-event-bus)
├─ EventId + EventEnvelope<T>
├─ EventBus trait and in-memory bus
├─ OutboxEntry + OutboxStore
├─ OutboxRelay + retry/backoff loop
├─ Outbox metrics and tracing spans
├─ SQLite outbox adapter
└─ Postgres outbox adapter
```

## Configuration

See [`.env.example`](.env.example) for the current environment variables.

Important values:

- `RUST_LOG`
- `EVENTKIT_LOG_FORMAT`
- `EVENTKIT_HEALTH_PORT`
- `EVENTKIT_HEALTHCHECK_TIMEOUT_MS`
- `OUTBOX_POLL_INTERVAL_MS`
- `OUTBOX_BATCH_SIZE`
- `OUTBOX_SHUTDOWN_TIMEOUT_SECS`

## Docs

- [`docs/deploy.md`](docs/deploy.md) for deployment and health-check guidance
- [`docs/disposition/phenotype-event-bus-runtime-boundary.md`](docs/disposition/phenotype-event-bus-runtime-boundary.md) for the runtime boundary decision
- [`SPEC.md`](SPEC.md) for the higher-level framework specification

## License

MIT OR Apache-2.0
