# eventkit - Event-Driven Architecture Framework

> **Work state:** ACTIVE · **Progress:** `█████░░░░░ 50%`
> Event-driven Rust framework: CQRS + Event Sourcing with EventStore and projection support. Hexagonal architecture; adapters for in-memory, Postgres, Kafka, RabbitMQ. · updated 2026-06-08

CQRS and Event Sourcing with EventStore and projection support.

## Features

- **Event Sourcing**: Store events, not state
- **CQRS**: Separate read/write models
- **Event Store**: Append-only event storage
- **Projections**: Build read models from events
- **Snapshots**: Optimize state reconstruction
- **Upcasting**: Handle event schema evolution

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      HEXAGONAL ARCHITECTURE                  │
├─────────────────────────────────────────────────────────────┤
│  Domain Layer                                                │
│  ├── Event (entity)                                         │
│  ├── Aggregate (entity)                                     │
│  ├── Command (value object)                                 │
│  └── EventStore trait (port)                               │
├─────────────────────────────────────────────────────────────┤
│  Application Layer                                           │
│  ├── CommandHandler (use case)                             │
│  ├── EventBus (use case)                                   │
│  └── ProjectionManager (use case)                          │
├─────────────────────────────────────────────────────────────┤
│  Adapters                                                    │
│  ├── InMemoryEventStore, PostgresEventStore                  │
│  ├── KafkaEventBus, RabbitMQEventBus                        │
│  └── ProjectionRunner                                       │
└─────────────────────────────────────────────────────────────┘
```

## Usage

```rust
use eventkit::{Aggregate, Event, Command};

let aggregate = AccountAggregate::new("acc-1");
aggregate.execute(Command::Deposit { amount: 100.0 })?;

let events = aggregate.uncommitted_events();
```

## License

MIT OR Apache-2.0
