//! Infrastructure Layer — intentionally thin.
//!
//! Cross-cutting infrastructure concerns (logging, metrics, health checks,
//! OTel export) are implemented in the **`eventkit-obs`** standalone crate
//! (`rust/eventkit-obs/`). That crate ships its own binary (`healthcheck`)
//! and library surface and is not yet a workspace member (see scope-reduction
//! ledger item 5 in `AGENTS.md`).
//!
//! This module is kept as a public namespace so downstream crates can do
//! `use eventkit::infrastructure` without a breaking change once obs helpers
//! are wired in. It intentionally exports nothing today — add items here only
//! when they genuinely belong to the root `eventkit` crate rather than to
//! `eventkit-obs`.
