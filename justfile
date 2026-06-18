# justfile — Eventra
# Standard Phenotype org recipes. Run `just` to list.

set shell := ["bash", "-uc"]

# Default recipe — list available recipes
default:
	@just --list

# Build
build:
	cargo build --workspace

# Run tests
test:
	cargo test --workspace

# Coverage report (SSOT for how to measure coverage).
coverage:
	cargo tarpaulin --workspace

# Lint
lint:
	cargo clippy --workspace --all-targets -- -D warnings
	cargo fmt --all -- --check

# Format
fmt:
	cargo fmt --all

# Security audit
audit:
	cargo audit

# Quality gate (build + test + lint + audit)
quality: build test lint audit
	@echo "quality gate OK"

# Clean
clean:
	cargo clean
