<!-- Work-state: alpha — [##--------] 25% — ports defined, in-mem store + bus, no persistence, no tests -->

<!-- AI-DD-META:START -->
<!-- This repository is planned, maintained, and managed by AI Agents only. -->
<!-- Slop issues are expected and intentionally present as part of an HITL-less -->
<!-- /minimized AI-DD metaproject of learning, refining, and building brute-force -->
<!-- training for both agents and the human operator. -->
![Downloads](https://img.shields.io/github/downloads/KooshaPari/Eventra/total?style=flat-square&label=downloads&color=blue)
![GitHub release](https://img.shields.io/github/v/release/KooshaPari/Eventra?style=flat-square&label=release)
![License](https://img.shields.io/github/license/KooshaPari/Eventra?style=flat-square)
![AI-Slop](https://img.shields.io/badge/AI--DD-Slop%20Expected-orange?style=flat-square)
![AI-Only-Maintained](https://img.shields.io/badge/Planned%20%26%20Maintained%20by-AI%20Agents%20Only-red?style=flat-square)
![HITL-less](https://img.shields.io/badge/HITL--less%20AI--DD-metaproject-yellow?style=flat-square)

> ⚠️ **AI-Agent-Only Repository**
>
> This repo is **planned, maintained, and managed exclusively by AI Agents**.
> Slop issues, rough edges, and AI artifacts are **expected and intentionally
> present** as part of an **HITL-less / minimized AI-DD** metaproject focused
> on learning, refining, and brute-force training both the agents and the
> human operator. Bug reports and contributions are still welcome, but please
> expect AI-generated code, comments, and documentation throughout.
<!-- AI-DD-META:END -->
# eventkit - Event-Driven Architecture Framework

## Quickstart

> Event-driven architecture framework with CQRS and Event Sourcing

```bash
# Clone, build, test
git clone https://github.com/KooshaPari/Eventra.git
cd Eventra
```

```rust
// Add to Cargo.toml:
// eventra = "<version>"
```

See [SPEC.md](SPEC.md) for the full specification and [llms.txt](llms.txt) for machine-readable metadata.


> **Work state:** ACTIVE · **Progress:** `█████░░░░░ 50%`
> Event-driven Rust framework: CQRS + Event Sourcing with EventStore and projection support. Hexagonal architecture; in-memory adapters shipped today; Postgres/Kafka/RabbitMQ adapters planned. · updated 2026-06-18

> **Status:** v0.1.0 — pre-release, in-memory adapters only. Postgres/Kafka/RabbitMQ adapters planned.

CQRS and Event Sourcing with EventStore and projection support.

## Features

- **Event Sourcing**: Store events, not state
- **CQRS**: Separate read/write models
- **Event Store**: Append-only event storage
- **Projections**: Build read models from events

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
│  ├── InMemoryEventStore                                       │
│  ├── InMemoryEventBus                                         │
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
