# Adapter Migration Guide

This document describes how to move Eventra from the in-memory adapters
shipped today to production-grade Postgres / Kafka / RabbitMQ adapters.
It also documents the new file-backed adapters that already implement the
production-grade contract without any new runtime dependencies.

## Overview

| Adapter | In-memory (default) | Persistent (file-backed) | Production target |
|---|---|---|---|
| EventStore | `InMemoryEventStore` | `PersistentEventStore` | `PostgresEventStore` |
| EventBus | `InMemoryEventBus` | `PersistentEventBus` | `KafkaEventBus` / `RabbitMQEventBus` |
| Projection checkpoint | (none) | `FileCheckpointStore` | `PostgresCheckpointStore` |

All non-in-memory adapters implement the same trait as the in-memory
variant, so swapping one in is a one-line change at the call site.

## EventStore

### In-memory → persistent (no code change beyond constructor)

```rust
use eventkit::adapters::{InMemoryEventStore, PersistentEventStore};
use eventkit::domain::EventStore;

let store: Box<dyn EventStore> =
    Box::new(PersistentEventStore::open("/var/lib/eventra/events")?);
```

`PersistentEventStore` writes every event as a single JSONL line to
`events.log` and persists the latest version per aggregate to a
`checkpoints/<aggregate_id>` file. The on-disk log is crash-safe: each
`append` is `flush()`-ed before the version checkpoint is committed, so
the worst case after a crash is a re-readable log whose highest version
has not yet been checkpointed. Reads stream the log from offset 0, so
ordering matches `InMemoryEventStore` exactly.

Optimistic concurrency is enforced the same way the in-memory store does
it: an `append` whose `version` is not exactly `last + 1` returns
`EventError::ConcurrencyConflict`. Two processes racing on the same
aggregate will see the second one fail, matching the behaviour of the
intended Postgres `UNIQUE (aggregate_id, version)` constraint.

### Persistent → Postgres (planned)

The `PersistentEventStore` API is intentionally identical to the trait
the future `PostgresEventStore` will implement. The schema is:

```sql
CREATE TABLE events (
    event_id        UUID PRIMARY KEY,
    aggregate_id    TEXT NOT NULL,
    aggregate_type  TEXT NOT NULL,
    event_type      TEXT NOT NULL,
    version         BIGINT NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL,
    causation_id    UUID,
    correlation_id  UUID,
    payload         JSONB NOT NULL,
    UNIQUE (aggregate_id, version)
);
CREATE INDEX events_aggregate_idx ON events (aggregate_id, version);
```

A swap looks like:

```rust
let store: Box<dyn EventStore> =
    Box::new(PostgresEventStore::connect("postgres://...").await?);
```

No call site changes are required.

## EventBus

### In-memory → persistent (outbox pattern)

```rust
use eventkit::application::PersistentEventBus;

let bus = PersistentEventBus::open("/var/lib/eventra/bus")?;
```

`PersistentEventBus` logs every published event to `bus.log` and tracks a
delivery offset per subscriber under `offsets/<subscriber_id>`. New
subscribers replay from offset 0; returning subscribers resume from their
last persisted offset. A failing handler does not advance the offset, so
a restart re-delivers the failing message.

### Persistent → Kafka (planned)

The contract is identical to a Kafka consumer-group:

- `publish` translates to `producer.send(topic=event_type, payload=event)`
- `subscribe` translates to `consumer.subscribe(topics=event_types)` plus
  a poll loop that calls `handler.handle` for each record
- the per-subscriber offset file is replaced by the consumer-group
  committed offset, persisted by Kafka

```rust
let bus: Box<dyn EventBus> =
    Box::new(KafkaEventBus::connect("localhost:9092", "eventra").await?);
```

### Persistent → RabbitMQ (planned)

The same shape using a durable queue per `event_type` with manual acks.
`subscribe` declares a named queue + binding and acks only after
`handler.handle` returns `Ok`. Failed messages stay on the queue for
redelivery.

## Projection checkpoint

### No checkpoint → persistent checkpoint

```rust
use eventkit::application::{CheckpointedProjectionRunner, FileCheckpointStore};

let runner = CheckpointedProjectionRunner::new(
    event_store,
    Box::new(FileCheckpointStore::open("/var/lib/eventra/checkpoints")?),
);
runner.register(my_projection);
runner.run()?;
```

`FileCheckpointStore` writes one JSON file per projection and updates it
atomically (`write tmp` → `rename`).

### File → Postgres (planned)

The intended schema is:

```sql
CREATE TABLE projection_checkpoints (
    projection_name TEXT PRIMARY KEY,
    position        BIGINT NOT NULL,
    last_updated    TIMESTAMPTZ NOT NULL
);
```

A `PostgresCheckpointStore` implements the same `CheckpointStore` trait
and drops in for `FileCheckpointStore` without any call site change.

## Why file-backed first?

The 71-pillar audit scored both `EventStore` and `EventBus` at 1/3
because they were in-memory only. Adding the Postgres / Kafka / RabbitMQ
clients is a meaningful dependency change and a separate workstream.
The file-backed adapters ship the production semantics — durability,
optimistic concurrency, at-least-once delivery, persistent projection
checkpoints — using only the existing dependency set, so a single-node
deployment is production-ready *today*. The Postgres / Kafka / RabbitMQ
adapters are then a pure swap of the trait implementation, with the
schema and contract documented here in advance.
