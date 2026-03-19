# Testing Strategy

Every PR must maintain or improve test coverage. CI fails if any test fails.

See [ADR-0009](../decisions/0009-testing-strategy.md) for the full rationale.

## Backend (Rust)

### Unit Tests

Location: `#[cfg(test)]` modules and `tests/unit/`

- No database, no network.
- Pure functions: stats math, crypto operations, data transformations, route handlers with mocked DB.
- Run: `cargo test --lib`

### Integration Tests

Location: `tests/integration/`

- `testcontainers-rs` spins up an ephemeral Postgres container per test module via `common::setup_db()`.
- External APIs mocked with `wiremock`. Fixtures in `tests/fixtures/<source>/`.
- Fully parallel-safe -- no shared state.
- Run: `cargo test --test integration`

### Contract Tests

Location: `tests/contract/`

- `pact_verifier` reads `pact/contracts/*.json`.
- Spins up the API against a testcontainers Postgres.
- Verifies the backend satisfies all consumer contracts.
- Run: `cargo test --test contract`

### Additional Checks

- `cargo clippy -- -D warnings` -- no warnings allowed.
- `cargo sqlx prepare --check` -- SQLx offline query data must be up to date.

## Web (React + TypeScript)

### Unit and Component Tests

Framework: Vitest + React Testing Library + MSW (Mock Service Worker)

- API calls mocked via MSW.
- Run: `npm test`

### E2E Tests

Framework: Playwright

- Tests run against a real backend with testcontainers Postgres.
- Run: `npm run test:e2e`

### Type Checking

- `tsc --noEmit` must pass.
- Run: `npm run type-check`

## iOS (Swift)

### Unit Tests

Framework: Swift Testing (Xcode 16)

- Parallel by default.
- HealthKit abstracted behind `HealthKitProvider` protocol; use `MockHealthKitProvider` in tests.
- Network abstracted behind `NetworkClient` protocol; mock in tests.
- SwiftUI views tested with ViewInspector.
- Run: `xcodebuild test -scheme OwnPulse -destination 'platform=iOS Simulator,name=iPhone 16'`

### E2E Tests

Framework: Maestro

- YAML-based flows in `ios/maestro/flows/`.
- Flows are deterministic: use `assertVisible` to confirm state before acting.
- Run: `maestro test ios/maestro/flows/`

## Contract Tests (Cross-Service)

Consumer contracts live in `pact/contracts/`:
- `web-backend.json` -- what the web frontend expects from the API
- `ios-backend.json` -- what the iOS app expects from the API

When adding or changing an API endpoint:
1. Check if the endpoint is referenced in a contract.
2. If yes: update the contract and run `cargo test --test contract`.
3. If adding a new endpoint used by iOS or web: add it to the appropriate contract.

## Test Data

- Use the `fake` crate (Rust) for realistic test data.
- Use `rstest` for parameterized tests.
- Never hardcode UUIDs or timestamps.
- WireMock fixtures in `backend/tests/fixtures/<source>/` are recorded once from real APIs and committed.
