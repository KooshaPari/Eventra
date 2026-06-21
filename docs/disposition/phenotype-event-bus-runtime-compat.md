# phenotype-event-bus runtime boundary

Date: 2026-06-21
Decision: KEEP_COMPAT
Canonical runtime bus owner: KooshaPari/phenoEvents

## Context

Eventra owns the CQRS/event-sourcing framework boundary for the Phenotype ecosystem.
phenoEvents owns the reusable runtime event-bus substrate.

This repository still contains three event-related surfaces with different jobs:

| Surface | Evidence | Ownership |
| --- | --- | --- |
| Framework-local in-memory bus | src/application/event_bus.rs | Eventra-owned adapter for Eventra domain events. |
| Event contracts | rust/phenotype-event-contracts | Eventra-owned trait and envelope contracts. |
| Event sourcing | rust/phenotype-event-sourcing | Eventra-owned event-sourcing package. |
| Runtime bus crate | rust/phenotype-event-bus | Compatibility surface; not canonical for new runtime bus work. |

## Decision

Keep rust/phenotype-event-bus as a compatibility crate for now. Do not delete it as a duplicate of phenoEvents until its public install/import surface has either been migrated or intentionally retired in a breaking release.

New runtime bus features belong in phenoEvents, not in rust/phenotype-event-bus.

## Rationale

rust/phenotype-event-bus is not a trivial duplicate. It exposes a distinct public API:

- generic EventEnvelope<T> payloads;
- EventId based on ULID;
- subject-based subscriptions;
- wildcard subject routing;
- async publish, subscribe, request, and close operations.

phenoEvents is the stronger canonical runtime bus because it carries the maintained substrate responsibilities: durable SQLite outbox, dead-letter handling, idempotency, projection-oriented envelope handling, metrics, and tracing.

Eventra should retain its framework-local bus and contracts because those are coupled to CQRS/event-sourcing semantics rather than a general reusable runtime bus substrate.

## Required gates before removal

Before rust/phenotype-event-bus can be removed from Eventra:

1. Scan GitHub and local repos for phenotype-event-bus imports, package references, and lockfile references.
2. Decide whether subject/wildcard routing remains a required feature; if yes, add it to phenoEvents or publish a small adapter crate.
3. Provide a migration guide from EventEnvelope<T> and ULID IDs to the canonical phenoEvents envelope model.
4. Mark the crate deprecated in one release or make a major-version breaking change.
5. Update phenotype-registry with the final disposition.

## Current allowed work

Allowed:

- security fixes;
- compatibility fixes;
- docs clarifying the ownership boundary;
- adapters that point consumers toward phenoEvents.

Not allowed:

- new standalone runtime bus features;
- expanding phenotype-event-bus as a second canonical bus;
- deleting the crate without the gates above.
