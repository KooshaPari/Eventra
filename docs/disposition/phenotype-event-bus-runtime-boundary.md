# phenotype-event-bus runtime boundary

Date: 2026-06-21
Status: KEEP_COMPAT
Owner: Eventra framework boundary

## Decision

Eventra keeps `rust/phenotype-event-bus` as a compatibility crate for its existing
dynamic install/import surface, but it is not the canonical place for new runtime
event-bus substrate work.

New reusable runtime event-bus features belong in `phenoEvents`. Eventra owns the
CQRS/event-sourcing framework, event contracts, and framework-local in-memory
adapter surfaces.

## Evidence

| Surface | Evidence | Disposition |
| --- | --- | --- |
| Eventra framework-local bus | `src/application/event_bus.rs` implements synchronous in-process publish/subscribe over Eventra domain `Event` and `EventHandler`. | Keep as framework-local CQRS adapter. |
| Eventra event contracts | `rust/phenotype-event-contracts/src/{bus,pubsub,store,envelope}.rs` define traits and Eventra envelopes. | Keep as Eventra-owned contracts. |
| Eventra event sourcing | `rust/phenotype-event-sourcing` is framework event-store / aggregate support. | Keep as Eventra-owned framework package. |
| Eventra runtime bus crate | `rust/phenotype-event-bus/src/lib.rs` exposes `EventId(Ulid)`, generic `EventEnvelope<T>`, subject subscriptions, request/close APIs, and `memory.rs` wildcard routing. | Keep compatibility; no new substrate features. |
| Canonical reusable runtime bus | `../phenoEvents/src/bus/mod.rs` exposes `Bus`, `Ack`, `Subscription`, `InMemoryBus`, and `SqliteBus` with duplicate detection, durable outbox, retries, DLQ, metrics/tracing. | Canonical substrate for new runtime bus work. |

## Why this is not a direct deletion

`phenotype-event-bus` is a separately installable/importable crate with a public API
that is not at parity with `phenoEvents`:

- Generic payload API: `EventEnvelope<T>`.
- ULID event identifiers.
- Subject and wildcard routing semantics.
- Request/response and explicit close methods in the trait surface.

Deleting or removing it from the workspace would risk breaking downstream consumers
without a dependency/import scan and without a major-version migration path.

## Forward rule

- Add new reusable runtime-bus capabilities to `phenoEvents`.
- Add Eventra CQRS/event-sourcing behavior to Eventra packages.
- Keep `phenotype-event-bus` limited to compatibility fixes, security fixes, and
  adapters that help consumers move to `phenoEvents`.
- Do not add durable outbox, DLQ, projection, retry, schema-registry, or observability
  substrate features to `phenotype-event-bus`; those belong in `phenoEvents`.

## Removal gate

Before `rust/phenotype-event-bus` can be archived or removed:

1. Scan GitHub and local repos for imports, Cargo dependencies, package references,
   and README install snippets for `phenotype-event-bus`.
2. Provide a compatibility path: adapter, re-export, or explicit major-version
   deprecation.
3. Preserve the subject/wildcard routing behavior or document why it is intentionally
   retired.
4. Update `phenotype-registry` with a traceability row showing target evidence in
   `phenoEvents`.
5. Cut a release or tag that makes the boundary decision discoverable to package
   consumers.
