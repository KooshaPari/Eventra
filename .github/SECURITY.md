# Security Policy

Thank you for helping keep **Eventra** and the broader **Phenotype** ecosystem safe.

This document is the canonical security policy for this repository. It
complements the project's license, contribution guide, and dependabot
configuration. GitHub displays this file under the repository's
**Security → Policy** tab.

---

## 1. Supported Versions

| Branch    | Supported          | Notes                                  |
| --------- | ------------------ | -------------------------------------- |
| `main`    | ✅ Active          | Receives security fixes and audits.    |
| `master`  | ✅ Mirrored alias  | Same as `main`; CI runs on both.       |
| other     | ❌ Best-effort     | No backports; please upgrade.          |

Eventra follows [semver](https://semver.org/). Until `1.0.0` the API is
considered unstable, but **security advisories are still honored** on the
latest minor release and on `main`.

---

## 2. Reporting a Vulnerability

**Please do not file public issues for security bugs.**

Report privately via one of the following channels (in order of preference):

1. **GitHub private vulnerability reporting**
   Go to the repository's **Security → Advisories → "New draft security
   advisory"** tab. This routes the report to the maintainers without
   disclosing it publicly.
2. **Email**
   Send to the address listed on the maintainer's GitHub profile
   ([@KooshaPari](https://github.com/KooshaPari)). Encrypt sensitive
   details with the maintainer's PGP key when available.
3. **Coordinated disclosure**
   If you need a secure channel outside GitHub, open a regular issue
   requesting contact and we will switch to a private channel.

### What to include

- A clear description of the issue and its impact.
- A reproducer (code snippet, command, or PoC).
- The affected commit SHA, tag, or release.
- Your assessment of severity (CVSS if possible).
- Any known workarounds.

### What to expect

| Step                                | Target SLA          |
| ----------------------------------- | ------------------- |
| Initial acknowledgement             | within 72 hours     |
| Triage & severity assessment        | within 7 days       |
| Patch released or status update     | within 30 days      |
| Public advisory (CVE / GHSA)        | after fix is merged |

We follow [coordinated disclosure](https://en.wikipedia.org/wiki/Coordinated_vulnerability_disclosure):
we ask reporters to give us a reasonable window to patch before publishing
details. We will credit reporters in the advisory unless they prefer
anonymity.

---

## 3. Automated Security Tooling

This repository runs the following automated checks on every push,
pull request, and weekly schedule:

- **[`audit.yml`](./workflows/audit.yml)** — `rustsec/audit-check` (cargo
  audit), `npm audit` (when a `package-lock.json` is present), and
  `pip-audit` (when Python manifests are present).
- **[`deny.yml`](./workflows/deny.yml)** — `cargo-deny` (advisories,
  license, source, duplicate detection) and a Go module sanity check.
- **[`scorecard.yml`](./workflows/scorecard.yml)** — OSSF Scorecard
  supply-chain analysis, results published as SARIF.
- **[`dependabot.yml`](./dependabot.yml)** — weekly grouped dependency
  updates for cargo, pip, npm, gomod, and GitHub Actions.
- **[`release-attestation.yml`](./workflows/release-attestation.yml)** —
  signed build provenance for release artifacts.

CI results are surfaced in pull-request checks. A failure on `main` is
treated as a release blocker.

---

## 4. Hardening Guidelines for Contributors

When contributing code that handles untrusted input, network I/O, crypto,
secrets, or persistence:

- **No `unsafe`** unless absolutely justified and reviewed; current
  code in this crate is 100% safe Rust.
- **No panics on the hot path** — return `Result` and propagate via
  `?`. Use `thiserror` for typed error enums.
- **No blocking calls in async contexts** — use `tokio` adapters and
  `async-trait` for trait objects.
- **No `unwrap` / `expect` on user-controlled data** — validate at the
  boundary and bubble up errors.
- **No new direct git dependencies** without justification in the PR
  description; cargo-deny will reject unknown git sources.
- **Cryptography** — prefer well-audited crates (`sha2`, `hmac`,
  `chacha20poly1305`, `aes-gcm`). Do not roll your own primitives.
- **Logging** — never log secrets, tokens, or PII. Use structured
  fields via `tracing`.

---

## 5. Dependency Governance

- All third-party dependencies are declared in the workspace
  `Cargo.toml` and pinned via `Cargo.lock`.
- Cargo-deny is configured to:
  - allow only the official `crates.io` registry plus the
    `KooshaPari/phenotype-types` git source;
  - allow only permissive / widely-used open-source licenses
    (MIT, Apache-2.0, BSD-2/3, ISC, Zlib, Unicode, MPL-2.0, etc.);
  - fail on advisories, yanked crates, and duplicate versions.
- Major-version bumps of dependencies are reviewed manually and
  delivered in their own PR with a migration note.

---

## 6. Supply-Chain & Provenance

- Release artifacts are built by the `release-attestation.yml`
  workflow with [SLSA](https://slsa.dev)-style provenance and signed
  with the maintainer's key.
- Scorecard is run on a weekly cron and after workflow changes; results
  are uploaded to the repository's **Security → Code scanning** tab.

---

## 7. Recognition

We are grateful to the security community. Reporters who follow this
policy will be credited in:

- the GitHub Security Advisory,
- the release notes for the fix, and
- this `SECURITY.md` "Hall of Fame" (with consent).

---

## 8. Scope

This policy covers:

- Source code in this repository and its workspace members under
  `rust/` (`phenotype-event-contracts`, `phenotype-event-bus`,
  `phenotype-event-sourcing`).
- CI workflows under `.github/workflows/`.
- The `deny.toml`, `dependabot.yml`, and `SECURITY.md` configuration.

Out of scope (please report upstream):

- Vulnerabilities in third-party crates — open an issue here only if
  the crate is a direct git dependency of Eventra. Otherwise, report
  to the upstream crate's maintainers and to
  [RustSec](https://rustsec.org/).
- Issues in tooling hosted outside this repository (e.g. GitHub
  Actions maintained by third parties).

---

_Last reviewed: 2026-06-20 — maintained by @KooshaPari._
