# 71-Pillar Scorecard — Eventra

**Date:** 2026-06-20
**Cycle:** 4
**Repo:** Eventra (eventkit — CQRS/ES framework)
**Branch:** fix/architectural-cleanup-2026-06-18
**Commit:** `HEAD`

---

## Methodology

Each of the 7 core architectural pillars is scored 0–3 across 9 cross-cutting domains.
Scores reflect the state of the codebase at the time of this audit.

**Scoring scale:**
- **0** — Not present / not implemented
- **1** — Minimal / skeleton with gaps
- **2** — Solid implementation with tests, docs, and error handling
- **3** — Production-ready with comprehensive coverage, observability, and hardening

---

## Score Matrix

| Pillar | Architecture | Performance | Quality | DX | UX | Security | Observability | Documentation | Governance | **Avg** |
|--------|:-----------:|:----------:|:-------:|:--:|:--:|:--------:|:------------:|:------------:|:---------:|:------:|
| **1. Event** | 2 | 2 | 2 | 2 | 2 | 1 | 1 | 2 | 2 | **1.78** |
| **2. Aggregate** | 2 | 2 | 2 | 2 | 2 | 1 | 1 | 1 | 2 | **1.67** |
| **3. Command** | 2 | 2 | 2 | 2 | 2 | 1 | 1 | 1 | 2 | **1.67** |
| **4. EventStore** | 2 | 1 | 2 | 2 | 2 | 1 | 1 | 2 | 2 | **1.67** |
| **5. EventBus** | 2 | 1 | 2 | 2 | 2 | 1 | 1 | 2 | 2 | **1.67** |
| **6. Projection** | 2 | 2 | 2 | 2 | 2 | 1 | 1 | 2 | 2 | **1.78** |
| **7. Contracts** | 2 | 2 | 2 | 2 | 2 | 1 | 1 | 2 | 2 | **1.78** |
| **Avg** | **2.00** | **1.71** | **2.00** | **2.00** | **2.00** | **1.00** | **1.00** | **1.71** | **2.00** | **1.71** |

---

## Pillar Details

### 1. Event (`src/domain/event.rs`, `rust/phenotype-event-contracts/src/event.rs`)

**Score: 1.78/3 (3rd highest — tied)**

| Domain | Score | Rationale |
|--------|:-----:|-----------|
| Architecture | 2 | Well-designed `Event` + `EventMetadata` structs. Trait-based with `EventBus` and `EventHandler` ports defined in domain layer. Split between main crate and contracts subcrate. |
| Performance | 2 | Simple struct with serde serialization. No performance concerns. |
| Quality | 2 | Tested via aggregate unit tests. Error handling through `EventError`. |
| DX | 2 | Builder methods (`with_causation_id`, `with_correlation_id`). Clear constructors. |
| UX | 2 | Clear abstractions for framework consumers. |
| Security | 1 | No input validation on event payloads. `serde_json::Value` is untyped and accepts arbitrary JSON. |
| Observability | 1 | No tracing, logging, or metrics instrumentation on event creation or publication. |
| Documentation | 2 | Module-level docs present. Traits have doc comments. |
| Governance | 2 | Follows hexagonal architecture patterns. Port/trait defined in domain. |

**Improvements needed:** Add input validation, observability instrumentation, typed event payload support.

---

### 2. Aggregate (`src/domain/aggregate.rs`)

**Score: 1.67/3 (joint lowest)**

| Domain | Score | Rationale |
|--------|:-----:|-----------|
| Architecture | 2 | Clean `Aggregate` trait with default `load_from_events` impl. `BaseAggregate` provides reusable scaffolding. |
| Performance | 2 | Simple in-memory state management. No concerns. |
| Quality | 2 | 5 unit tests covering basic ops, versioning, event commitment, trait delegation, and event replay. |
| DX | 2 | Trait with sensible defaults. Easy to implement custom aggregates. |
| UX | 2 | `execute` method returns `Result<Vec<Event>>`. Clean API. |
| Security | 1 | No state validation guards. `apply` blindly increments version. |
| Observability | 1 | No logging of aggregate operations (apply, load, execute). |
| Documentation | 1 | Module-level doc comment present but sparse. No example usage. |
| Governance | 2 | Good separation of concerns. |

**Improvements needed:** Add state validation in `apply`, observability instrumentation, richer documentation with examples.

---

### 3. Command (`src/domain/command.rs`, `src/application/command_handler.rs`)

**Score: 1.67/3 (joint lowest)**

| Domain | Score | Rationale |
|--------|:-----:|-----------|
| Architecture | 2 | Command value object + `CommandHandlerService` + `AggregateFactory` pattern. Good separation of concerns. |
| Performance | 2 | Simple object, no concerns. |
| Quality | 2 | One integration test for `handle_rehydrates_executes_and_persists_new_events`. |
| DX | 2 | Generic `CommandHandlerService<A>` with factory pattern. |
| UX | 2 | Clean command execution flow. |
| Security | 1 | No command validation or authorization checks. |
| Observability | 1 | No tracing through command handler pipeline. |
| Documentation | 1 | Minimal docs on Command struct. `CommandHandlerService` lacks detailed docs. |
| Governance | 2 | Follows CQRS pattern correctly. |

**Improvements needed:** Command validation, authorization hooks, tracing spans, better docs.

---

### 4. EventStore (`src/domain/event.rs` trait, `src/adapters/event_store.rs` impl)

**Score: 1.67/3 (joint lowest)**

| Domain | Score | Rationale |
|--------|:-----:|-----------|
| Architecture | 2 | Port/adapter pattern. Trait in domain, impl in adapters. Concurrency version check. |
| Performance | 1 | In-memory only. No Postgres, Kafka, or DB adapter. Needs production-grade storage. |
| Quality | 2 | Concurrency conflict detection tested. Tested indirectly via command_handler and projection tests. |
| DX | 2 | Clean trait with all necessary methods (`append`, `get_events`, `get_events_since`, `get_all_events`). |
| UX | 2 | Simple append/read interface. |
| Security | 1 | No encryption, access control, or audit logging on stored events. |
| Observability | 1 | No logging of appends, reads, or concurrency conflicts. |
| Documentation | 2 | Trait docs, adapter docs present. |
| Governance | 2 | Good port/adapter separation. |

**Improvements needed:** Database adapters (Postgres), encryption at rest, access control, audit logging, observability.

---

### 5. EventBus (`src/domain/event.rs` trait, `src/application/event_bus.rs` impl)

**Score: 1.67/3 (joint lowest)**

| Domain | Score | Rationale |
|--------|:-----:|-----------|
| Architecture | 2 | Clear port/adapter pattern. `EventBus` trait in domain, `InMemoryEventBus` impl in application layer. |
| Performance | 1 | In-memory only. No Kafka, RabbitMQ, or message broker adapter. |
| Quality | 2 | One unit test for subscribe/publish flow. Handler cloning via `EventHandlerClone`. |
| DX | 2 | Simple publish/subscribe API. Arc-based handler management. |
| UX | 2 | Clean event bus abstraction. |
| Security | 1 | No message-level security, encryption, or auth. |
| Observability | 1 | No tracing spans through publish/subscribe. No metrics for message counts. |
| Documentation | 2 | Module docs, trait docs present. |
| Governance | 2 | Good separation between port and implementation. |

**Improvements needed:** Message broker adapters (Kafka/RabbitMQ), observability (publish counts, latency), message security.

---

### 6. Projection (`src/application/projection.rs`)

**Score: 1.78/3 (3rd highest — tied)**

| Domain | Score | Rationale |
|--------|:-----:|-----------|
| Architecture | 2 | `Projection` trait + `ProjectionRunner` with `ProjectionState` position tracking. Well-designed. |
| Performance | 2 | Position-based replay avoids redundant processing. |
| Quality | 2 | 2 tests: empty-store noop, 3-events with correct counting and position tracking. |
| DX | 2 | `register` + `run` pattern. Position tracking built in. |
| UX | 2 | Clear projection abstraction for building read models. |
| Security | 1 | No projection isolation or sandboxing. |
| Observability | 1 | No logging of projection progress, errors, or position advances. |
| Documentation | 2 | Good module-level and method docs with example test helpers. |
| Governance | 2 | Clean separation. |

**Improvements needed:** Projection isolation, checkpoint persistence across restarts, observability, parallel projection support.

---

### 7. Contracts (`rust/phenotype-event-contracts/`)

**Score: 1.78/3 (3rd highest — tied)**

| Domain | Score | Rationale |
|--------|:-----:|-----------|
| Architecture | 2 | Clean trait contracts (`Contract`, `Event`, `EventStore`, `EventBus`, `PubSubBus`, `EventHandler`). ADR-ECO-014 decomposition done. |
| Performance | 2 | Trait-only, zero overhead. |
| Quality | 2 | 2 tests. `missing_docs` lint enforced. `clippy::all` enforced. |
| DX | 2 | Well-named traits with clear responsibilities. |
| UX | 2 | Consistent contract patterns across the ecosystem. |
| Security | 1 | No security considerations in contract design. |
| Observability | 1 | No hooks for observability in contracts. |
| Documentation | 2 | Missing docs lint ensures all public items are documented. Doc examples present. |
| Governance | 2 | Clear ownership documented. Terminal owner: Eventra. References ADR. |

**Improvements needed:** Observability hooks in contracts, security annotations, additional adapter contracts.

---

## Domain Summary

| Domain | Avg Score | Assessment |
|--------|:---------:|------------|
| Architecture | 2.00 | Solid hexagonal architecture. Good port/adapter separation. |
| Performance | 1.71 | Only in-memory adapters exist. No DB/broker adapters. |
| Quality | 2.00 | 18 total tests passing. Error types are thorough. |
| DX | 2.00 | Clean trait APIs with builder patterns. |
| UX | 2.00 | Clear abstractions. Easy to extend. |
| Security | 1.00 | **Critical gap.** No validation, auth, encryption, or access control anywhere. |
| Observability | 1.00 | **Critical gap.** No tracing, logging, or metrics across the entire framework. |
| Documentation | 1.71 | Module docs present but some areas sparse (Aggregate, Command). |
| Governance | 2.00 | ADR-driven decisions. Good patterns. |

---

## Lowest-Scoring Pillars (Identified Issues)

### Issue 1: Security (avg 1.00 across all pillars)
Security is universally scored at 1 across every pillar. The entire framework lacks:
- Input validation on event payloads and commands
- Authorization/access control
- Encryption at rest or in transit
- Message-level security in the event bus
- Command validation guards

### Issue 2: Observability (avg 1.00 across all pillars)
Observability is universally scored at 1 across every pillar. The entire framework lacks:
- Structured logging (`tracing` crate)
- OpenTelemetry spans for command handling, event publication, projection runs
- Metrics for event throughput, latency, error rates
- Projection progress logging

### Issue 3: Performance — In-Memory Only (avg 1.71, driven by storage/bus adapters)
Both EventStore and EventBus are in-memory only. The framework cannot be used in production without database and message broker adapters. Required:
- Postgres EventStore adapter
- Kafka/RabbitMQ EventBus adapter
- Persistent projection checkpoint storage

---

## Key Findings

### Strengths
- Clean hexagonal architecture with clear port/adapter separation
- Well-structured domain, application, and adapter layers
- Contract traits decomposed per ADR-ECO-014
- Concurrency version checking in EventStore
- Projection position tracking with state management
- All 18 tests passing with good error handling coverage

### Critical Gaps
1. **Security (avg 1.00)** — No input validation, authorization, encryption, or access control across all pillars
2. **Observability (avg 1.00)** — No tracing, structured logging, or metrics throughout the framework
3. **In-memory only** — No database (Postgres) or message broker (Kafka/RabbitMQ) adapters exist

### Recommended Action Items
1. Add structured logging (`tracing` crate) across all pillars
2. Add OpenTelemetry spans for key operations (command handling, event publishing, projection runs)
3. Add input validation for events, commands, and aggregate state changes
4. Implement Postgres EventStore adapter
5. Add concurrency hardening with optimistic locking tests

---

*Score prepared by Forge subagent W11-2-14 on 2026-06-20 for Cycle 4 of the 71-Pillar audit program.*
