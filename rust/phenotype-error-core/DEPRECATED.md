# DEPRECATED — DO NOT USE

This vendored copy of `phenotype-error-core` is deprecated as of 2026-09-01.

**Canonical home:** `KooshaPari/HexaKit` → `crates/phenotype-error-core/`

## Migration

In `Cargo.toml`, replace:

```toml
phenotype-error-core = { path = "rust/phenotype-error-core" }
```

with:

```toml
phenotype-error-core = { git = "https://github.com/KooshaPari/HexaKit", tag = "v0.1.0-phenotype-error-core" }
```

Or in a workspace, drop the vendored member and add a git dep:

```toml
[workspace.dependencies]
phenotype-error-core = { git = "https://github.com/KooshaPari/HexaKit" }
```

## Removal schedule

This directory will be deleted after **2026-10-01** (30-day deprecation window).
Track progress: `KooshaPari/phenotype-registry` → PR #541 + ecosystem-consolidation/dossier/TIER3-P2-ERROR-CORE.md
