# AGENTS.md — Eventra

Agent-readable canonical reference for the Eventra workspace.
Read this before writing any code, adding dependencies, or restructuring crates.

## Repo purpose

Eventra is a **Rust event-sourcing and outbox library** for the Phenotype fleet.
It is **not** a standalone service — it ships crates consumed by other services.
The Docker/K8s scaffolding exists only for integration-test convenience.

## Crate map (canonical)

| Crate | Path | Role |
|---|---|---|
| `eventkit` | `.` (root) | Top-level re-export facade + tracing bootstrap |
| `phenotype-event-contracts` | `rust/phenotype-event-contracts/` | Shared types: `Event`, `EventMetadata`, `EventStore` trait, `Snapshot`, hash chain |
| `phenotype-event-sourcing` | `rust/phenotype-event-sourcing/` | Aggregate root, in-memory store, snapshotting, hash-chain verification |
| `phenotype-event-bus` | `rust/phenotype-event-bus/` | `EventBus` trait, `InMemoryEventBus`, transactional outbox stack (`OutboxStore`, `OutboxRelay`), SQLite/Postgres feature flags |
| `phenotype-error-core` | `rust/phenotype-error-core/` | Vendored fork of upstream `phenotype-types` error primitives (upstream 404 — see comment in `Cargo.toml`) |
| `eventkit-obs` | `rust/eventkit-obs/` | Observability helpers (OTel, metrics, health endpoints). **Standalone** — not yet a workspace member; see scope-reduction ledger item 5. |

## Domain model decision

The **canonical** domain model lives in `phenotype-event-contracts` and `phenotype-event-sourcing`.
The legacy `src/domain/`, `src/application/`, and `src/adapters/` modules in the root crate exist for
backward compatibility and re-export the contracts crate types.
`src/infrastructure/mod.rs` is intentionally minimal — cross-cutting infra (logging, metrics) is
handled by `eventkit-obs`. Do not add business logic there.

## In-memory adapters

`InMemoryEventStore` (root crate `src/adapters/`) and `InMemoryEventBus` (`phenotype-event-bus`) are
**PoC/test-only** surfaces. They carry `#[doc(alias = "test-only")]` annotations and must not be
marketed as production-ready. For production persistence see the `postgres`/`sqlite` feature flags
on `phenotype-event-bus`.

## Dependency rules

- All new `[workspace.dependencies]` entries must appear in the root `Cargo.toml` and be referenced
  via `{ workspace = true }` in member `Cargo.toml` files.
- Never add a direct Git or path dependency to a crate outside this workspace without approval.
- `phenotype-error-core` is vendored — do not replace it with a crates.io pin without first verifying
  upstream is accessible again.

## Testing policy

- Unit tests live alongside source (`#[cfg(test)]` modules).
- Property tests use the standard `#[test]` harness (no proptest/quickcheck dep yet — use
  table-driven deterministic property checks until a fuzzing crate is added).
- Run `cargo test --all` before committing.
- Run `cargo clippy --all -- -D warnings` and `cargo fmt --all -- --check` before committing.

## CI gates (`.github/workflows/ci.yml`)

The CI pipeline runs: `fmt` → `clippy -D warnings` → `cargo deny` → `cargo test --all`.
SHA-pinned actions on `ubuntu-24.04` runners. See `.github/workflows/ci.yml` for the full matrix.

## Scope — what agents should NOT touch

- `.github/workflows/release-attestation.yml` — SLSA provenance; do not edit without security review.
- `.github/workflows/scorecard.yml` — OpenSSF Scorecard; SHA pinning is intentional.
- `Cargo.lock` — commit it; do not `.gitignore` it for a library workspace (keeps reproducible builds).

## Functional requirements (FR) summary

| ID | Requirement | Primary crate |
|---|---|---|
| FR-ES-001 | Append events with monotonic version enforcement | `phenotype-event-sourcing`, root `src/adapters/` |
| FR-ES-002 | Reconstruct aggregate state from event log | `phenotype-event-sourcing` |
| FR-ES-003 | Hash-chain integrity: each event hashes over its predecessor | `phenotype-event-sourcing::hash` |
| FR-ES-004 | Snapshot support to bound replay length | `phenotype-event-sourcing::snapshot` |
| FR-EB-001 | Publish/subscribe with wildcard subjects | `phenotype-event-bus::memory` |
| FR-EB-002 | Transactional outbox with at-least-once delivery | `phenotype-event-bus::outbox` |
| FR-EB-003 | Outbox relay with jittered exponential-backoff retry | `phenotype-event-bus::outbox_relay` |
| FR-OB-001 | Health endpoint and structured tracing for observability | `eventkit-obs` |

## Acceptance criteria (AC) for common changes

### Adding a new `EventStore` adapter
- Implement `EventStore` from `phenotype-event-contracts`.
- Gate behind a Cargo feature flag (mirrors `postgres`/`sqlite` pattern).
- Provide integration tests that exercise FR-ES-001 through FR-ES-003.

### Adding a new `EventBus` backend
- Implement `EventBus` from `phenotype-event-bus`.
- Do not use unbounded channels in the hot path; prefer bounded `tokio::sync::mpsc::channel(N)`.
- Provide at least publish/subscribe/close tests.

### Changing the hash-chain schema
- Update `phenotype-event-sourcing::hash::compute_hash` and its doc comment.
- Update the snapshot of the canonical hash in `hash.rs` deterministic test.
- Add a migration note in `CHANGELOG.md` (breaking change = semver bump).
