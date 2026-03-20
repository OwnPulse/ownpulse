# Backend Service — AGENTS.md

## Ownership

**Owns:** `backend/`, `db/migrations/`, `schema/`

**Does NOT own:** `web/`, `ios/`, `helm/`, `.github/`

## Build

```
cargo build
```

## Test

```
cargo test
```

## Interface to Other Services

- REST API defined in `docs/architecture/api.md`
- Pact provider verification in `tests/contract/`

## Common Patterns

- Axum 0.7 with SQLx 0.7
- Configuration via envy (environment variable deserialization)
- Logging/tracing via the `tracing` crate (never `println!`)
- Error types defined with `thiserror`
- Integration tests use testcontainers (no Docker Compose)
- External API calls are stubbed with WireMock in tests

## What NOT to Do

- **No `println!`** — use `tracing` macros (`tracing::info!`, `tracing::error!`, etc.).
- **No `#[allow(dead_code)]` or clippy suppression as permanent fixes** — fix the underlying issue instead.
- **No hitting real external APIs in tests** — use WireMock.
- **No Docker Compose** — testcontainers handles integration-test infrastructure.
- **No adding structured tables for user-defined data** — use the observations model.
- **No validating substance names** — they are free-form user input.
- **No writing HealthKit-sourced records to `healthkit_write_queue`** — those flow in one direction only.
- **No buffering exports in memory** — stream them.
