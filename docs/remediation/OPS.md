# Ops / deploy remediation (audit area K)

Additive operations hardening for Eventra. Container, config, and deploy docs were added; apply the diffs below for full CI/workspace integration.

## What was added

| Artifact | Purpose |
|---|---|
| `Dockerfile` | Multi-stage build, non-root user, `HEALTHCHECK`, `SIGTERM` |
| `docker-compose.yml` | Local deploy with healthcheck + 30s grace period |
| `.env.example` | Documented env vars (no secrets) |
| `docs/deploy.md` | Deploy runbook, K8s probe examples, graceful shutdown |

## Gap analysis

| Requirement | Status | Location |
|---|---|---|
| Multi-stage Dockerfile | **Added** | `Dockerfile` |
| Container HEALTHCHECK | **Added** | `Dockerfile`, `docker-compose.yml` |
| `.env.example` | **Added** | `.env.example` |
| Deploy documentation | **Added** | `docs/deploy.md` |
| Graceful shutdown notes | **Added** | `docs/deploy.md`, Dockerfile `STOPSIGNAL` |
| CI Docker build | **Pending** | diff below |
| K8s manifests | **Future** | template below |

---

## Diff 1 — Workspace build in Dockerfile (after OBS wiring)

Once `eventkit-obs` is in the workspace (see `OBSERVABILITY.md` Diff 1), simplify `Dockerfile` builder stage:

```diff
-FROM rust:1.75-bookworm AS builder
-
-WORKDIR /build
-
-# Cache dependencies
-COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
-COPY rust/eventkit-obs/Cargo.toml rust/eventkit-obs/Cargo.toml
-
-RUN mkdir -p rust/eventkit-obs/src/bin \
-    && echo "pub fn _stub() {}" > rust/eventkit-obs/src/lib.rs \
-    && echo "fn main() {}" > rust/eventkit-obs/src/bin/healthcheck.rs \
-    && cargo build --release -p eventkit-obs 2>/dev/null || true
-
-# Full source build (eventkit-obs is standalone until wired into workspace;
-# build directly from its crate directory)
-COPY rust/eventkit-obs/ rust/eventkit-obs/
-
-WORKDIR /build/rust/eventkit-obs
-RUN cargo build --release
+FROM rust:1.75-bookworm AS builder
+WORKDIR /build
+COPY . .
+RUN cargo build --release -p eventkit-obs
```

---

## Diff 2 — CI: build Docker image on main

**File:** `.github/workflows/ci.yml`

```diff
 jobs:
   test:
     runs-on: ubuntu-latest
     steps:
       - uses: actions/checkout@v4
       - name: Install Rust
         uses: dtolnay/rust-toolchain@stable
       - name: Run tests
         run: cargo test --all
+
+  docker:
+    runs-on: ubuntu-latest
+    needs: test
+    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
+    steps:
+      - uses: actions/checkout@v4
+      - name: Build image
+        run: docker build -t eventra/eventkit:ci .
+      - name: Smoke healthcheck
+        run: docker run --rm eventra/eventkit:ci
```

---

## Diff 3 — `justfile` deploy recipes

**File:** `justfile`

```diff
 # Clean
 clean:
 	cargo clean
+
+# Container build + smoke
+docker-build:
+	docker build -t eventra/eventkit:local .
+
+docker-smoke: docker-build
+	docker run --rm eventra/eventkit:local
+
+docker-up:
+	docker compose up --build
```

---

## Diff 4 — Graceful shutdown in outbox relay service

When running `phenotype-event-bus` relay as a long-lived process, wire shutdown:

```rust
use phenotype_event_bus::{new_shutdown_token, run_outbox_relay, RelayConfig, Shutdown};
use tokio::signal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    eventkit_obs::init_logging(None, None);
    let shutdown = new_shutdown_token();
    let shutdown_clone = shutdown.clone();

    tokio::spawn(async move {
        let _ = signal::ctrl_c().await;
        tracing::info!("shutdown signal received, draining outbox relay");
        shutdown_clone.trigger();
    });

    let timeout = std::env::var("OUTBOX_SHUTDOWN_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    run_outbox_relay(/* store, publisher, */ RelayConfig::default(), shutdown)
        .await?;

    tokio::time::sleep(std::time::Duration::from_secs(timeout)).await;
    Ok(())
}
```

Set orchestrator grace period ≥ `OUTBOX_SHUTDOWN_TIMEOUT_SECS` (default 30s).

---

## Diff 5 — `.dockerignore` (recommended new file)

```
target/
.git/
.grade-reports/
**/*.log
.env
```

---

## Kubernetes starter manifest (new file: `deploy/k8s/deployment.yaml`)

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: eventkit-health
spec:
  replicas: 1
  selector:
    matchLabels:
      app: eventkit-health
  template:
    metadata:
      labels:
        app: eventkit-health
    spec:
      terminationGracePeriodSeconds: 30
      containers:
        - name: eventkit-health
          image: eventra/eventkit:latest
          ports:
            - containerPort: 8080
          envFrom:
            - configMapRef:
                name: eventkit-config
          livenessProbe:
            exec:
              command: ["eventkit-healthcheck"]
            initialDelaySeconds: 10
            periodSeconds: 30
          readinessProbe:
            httpGet:
              path: /ready
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
          lifecycle:
            preStop:
              exec:
                command: ["/bin/sh", "-c", "sleep 5"]
```

---

## Verification checklist

- [ ] `docker build -t eventra/eventkit:local .` succeeds
- [ ] `docker run --rm eventra/eventkit:local` exits 0 (JSON health on stdout)
- [ ] `docker compose up` shows healthy container
- [ ] `.env.example` copied to `.env` for local dev (never commit `.env`)
- [ ] SIGTERM stops container within grace period without SIGKILL

## Security notes

- Image runs as non-root `eventkit` user
- No secrets in `.env.example` — use K8s Secrets / vault for `DATABASE_URL` etc.
- Pin base image digests in production (`debian:bookworm-slim@sha256:...`)
