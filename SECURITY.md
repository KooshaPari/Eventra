# Security Policy

The canonical security policy for this repository lives at
[`.github/SECURITY.md`](./.github/SECURITY.md). This top-level file is a
shorthand pointer for tools, scanners, and contributors who look at the
repository root. GitHub displays `.github/SECURITY.md` under the
**Security → Policy** tab; this file is shown when scanners or humans
look at the repo root.

For full details on supported versions, automated tooling
(`audit.yml`, `deny.yml`, `secret-scan.yml`, `scorecard.yml`,
`dependabot.yml`, `release-attestation.yml`), contributor hardening
guidelines, dependency governance, supply-chain / SLSA provenance, and
the security-advisory recognition policy, please read
[`.github/SECURITY.md`](./.github/SECURITY.md).

---

## TL;DR for reporters

1. **Do not** file public GitHub issues for security bugs. Public issues
   leak attack vectors before a fix is shipped and may put users at
   risk.
2. Use **GitHub private vulnerability reporting**
   (Security → Advisories → "New draft security advisory"). This routes
   the report to the maintainers without disclosing it publicly.
3. If GitHub private reporting is unavailable, email the maintainer
   listed on [@KooshaPari](https://github.com/KooshaPari)'s GitHub
   profile.
4. Include a description, reproducer, affected commit SHA / tag, and a
   severity estimate.

Acknowledgement target: **72 hours**. Triage target: **7 days**. Fix or
status update target: **30 days**.

_Last reviewed: 2026-06-20 — maintained by @KooshaPari._