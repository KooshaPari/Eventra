# syntax=docker/dockerfile:1

# Multi-stage Dockerfile for Eventra workspace.
# Builds the eventkit-healthcheck CLI and ships a minimal runtime image.

# -----------------------------------------------------------------------------
# Stage 1: Build
# -----------------------------------------------------------------------------
FROM rust:1.75-bookworm AS builder

WORKDIR /build

# Cache dependencies
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY rust/eventkit-obs/Cargo.toml rust/eventkit-obs/Cargo.toml

RUN mkdir -p rust/eventkit-obs/src/bin \
    && echo "pub fn _stub() {}" > rust/eventkit-obs/src/lib.rs \
    && echo "fn main() {}" > rust/eventkit-obs/src/bin/healthcheck.rs \
    && cargo build --release -p eventkit-obs 2>/dev/null || true

# Full source build (eventkit-obs is standalone until wired into workspace;
# build directly from its crate directory)
COPY rust/eventkit-obs/ rust/eventkit-obs/

WORKDIR /build/rust/eventkit-obs
RUN cargo build --release

# -----------------------------------------------------------------------------
# Stage 2: Runtime
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /usr/sbin/nologin eventkit
USER eventkit

COPY --from=builder /build/rust/eventkit-obs/target/release/eventkit-healthcheck /usr/local/bin/eventkit-healthcheck

ENV RUST_LOG=info,eventkit=debug \
    EVENTKIT_LOG_FORMAT=plain \
    EVENTKIT_HEALTHCHECK_TIMEOUT_MS=3000

EXPOSE 8080

# Liveness: process alive (library mode) or probe configured URL at runtime
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["eventkit-healthcheck"]

# Graceful shutdown: send SIGTERM; allow in-flight relay workers to drain.
# Orchestrators should use terminationGracePeriodSeconds >= 30 (see docs/deploy.md).
STOPSIGNAL SIGTERM

ENTRYPOINT ["eventkit-healthcheck"]
CMD []
