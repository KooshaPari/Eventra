# Contributing

Thanks for your interest in contributing to Eventra.

## Development

1. Install the Rust toolchain.
2. Run `cargo build --workspace`.
3. Run `cargo test --workspace`.
4. Run `cargo fmt --all` before committing.
5. Run `cargo clippy --workspace --all-targets -- -D warnings` before opening a pull request.

## Quality

Use the standard local quality recipe when `just` is available:

```bash
just quality
```

## Security

Do not open public issues for sensitive vulnerabilities. Follow `SECURITY.md`.
