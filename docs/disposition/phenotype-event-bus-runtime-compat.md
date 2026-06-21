# phenotype-event-bus runtime boundary disposition

Date: 2026-06-21
Status: KEEP_COMPAT
Owner: Eventra
Related canonical runtime substrate: phenoEvents

## Decision

phenoEvents is the canonical reusable runtime event-bus substrate for the Phenotype ecosystem.

Eventra remains the canonical CQRS/event-sourcing framework and event-contracts owner. The Eventra root in-process bus (src/application/event_bus.rs) remains framework-local because it is coupled to Eventra domain events, aggregate metadata, handler dispatch, and synchronous CQRS/eventkit test seams.

rust/phenotype-event-contracts remains Eventra-owned as trait-only contracts for Eventra event envelopes, stores, pub/sub handlers, and framework integration.

rust/phenotype-event-bus is not canonical for new runtime-bus work. It is kept as a compatibility crate until a consumer scan and migration plan proves it can be removed or replaced with a pheno-events adapter.

## Why this is not deleted now

The crate has a separate dynamic install/import surface and distinct API shape:

- package/crate surface: phenotype-event-bus
- generic envelope: EventEnvelope<T>
- event id type: EventId(Ulid)
- subject-based subscriptions, including wildcard suffix matching
- async publish, subscribe, request, and close API shape

Those behaviors are not proven to be exact equivalents of pheno-events today. phenoEvents has a stronger canonical runtime substrate model, especially durable SQLite outbox, retry/DLQ, idempotency, metrics, tracing, and EventEnvelope-shaped contracts. But stronger substrate ownership does not by itself prove safe deletion of a public compatibility crate.

## Allowed work

Allowed in rust/phenotype-event-bus:

- security fixes
- compatibility fixes
- documentation
- adapters or bridges that reduce migration risk
- major-version deprecation work after downstream consumers are inventoried

Not allowed:

- new runtime-bus features that compete with phenoEvents
- new persistence, DLQ, registry, observability, or projection features
- expanding the crate into a second canonical event substrate

## Migration path

1. Inventory all internal and public references to phenotype-event-bus.
2. Decide whether subject/wildcard routing is required in phenoEvents or should stay framework-local.
3. If consumers exist, add a compatibility adapter or documented major-version migration to pheno-events.
4. If no consumers exist, preserve the final API snapshot in phenotype-registry and remove the crate from the Eventra workspace in a breaking cleanup PR.
5. Keep phenotype-event-contracts in Eventra unless a separate contracts-owner ADR supersedes it.

## Deletion readiness status

rust/phenotype-event-bus is not deletion-ready.

Minimum safe deletion gate:

- registry traceability row for the crate
- consumer/import scan
- explicit decision on EventEnvelope<T>, EventId(Ulid), and subject/wildcard routing
- either a pheno-events adapter/reexport or a documented no-consumer finding
- breaking-change notice if the crate was ever published or imported externally
