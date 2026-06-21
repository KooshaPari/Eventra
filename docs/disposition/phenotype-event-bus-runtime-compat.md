# phenotype-event-bus runtime compatibility disposition

Date: 2026-06-21

## Decision

phenoEvents is the canonical runtime event-bus substrate for new Phenotype
event-bus work.

Eventra/rust/phenotype-event-bus is retained as a compatibility crate, not as
the canonical runtime bus owner. It must not receive new runtime-bus feature
expansion except compatibility fixes, security fixes, or migration adapters.

Eventra remains the canonical home for CQRS, event-sourcing, and event-contract
semantics.

## Evidence

- phenoEvents/src/bus/mod.rs defines the canonical runtime Bus port with
  publish acknowledgement, subscriptions, durable SQLite bus support, retry,
  dead-letter queue, idempotency, tracing, and metrics surfaces.
- phenoEvents/src/bus/in_memory.rs provides the canonical in-memory runtime
  adapter and explicitly records its lineage from the deleted phenotype-bus
  implementation.
- Eventra/src/application/event_bus.rs is framework-local CQRS infrastructure
  over Eventra domain events. It is synchronous and aggregate/event-type scoped,
  so it is not a replacement for the reusable runtime substrate.
- Eventra/rust/phenotype-event-contracts owns framework contracts such as
  event envelopes, pub/sub traits, and event-store traits. These remain
  Eventra-owned because they are part of the CQRS/event-sourcing model.
- Eventra/rust/phenotype-event-bus exposes a distinct dynamic install/import
  surface: generic EventEnvelope<T>, EventId(Ulid), subject and wildcard
  routing, request, and close APIs. That API is not currently equivalent to
  phenoEvents, so blind deletion would break consumers without proven parity.

## Boundary rules

1. New runtime event-bus features go to phenoEvents.
2. New CQRS/event-sourcing framework features go to Eventra.
3. Eventra root in-memory bus remains framework-local unless a later bridge is
   explicitly designed.
4. rust/phenotype-event-bus remains compatibility-only until a consumer scan
   and migration path prove it can be replaced.
5. Removal requires a major-version deprecation plan or a compatibility shim
   that preserves separate install/import behavior.

## Migration path

Before removing or repointing rust/phenotype-event-bus:

1. Scan the fleet and GitHub for imports, package references, and Cargo
   dependencies on phenotype-event-bus.
2. Decide whether pheno-events should gain subject/wildcard routing, or
   whether that behavior should remain intentionally retired.
3. Provide either a thin adapter/reexport crate or a documented major-version
   breaking release.
4. Update phenotype-registry with the final traceability matrix.
5. Only then remove the workspace member or replace the implementation.

## Deletion readiness

Current status: LAST_RESORT_EXCEPTION for deletion of
Eventra/rust/phenotype-event-bus.

Reason: the crate has a meaningful public API and dynamic install/import surface
that is only partially covered by phenoEvents.

Required action before deletion: consumer scan plus adapter, reexport, or
explicit breaking-release deprecation.
