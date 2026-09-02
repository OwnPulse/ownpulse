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

- `testcontainers-rs` shares one Postgres container per test binary; each test gets its own database, cloned from the pre-migrated `template_ownpulse` template via `common::setup()`.
- External APIs mocked with `wiremock`. Fixtures in `tests/fixtures/<source>/`.
- Fully parallel-safe -- no shared state.
- Run: `cargo test --test integration`

### Contract Tests

Location: `tests/contract/`

- `pact_verifier` reads `pact/contracts/*.json`.
- Spins up the API against a testcontainers Postgres.
- The iOS gate (`verify_ios_contract_verifiable_set`) replays every
  interaction listed in `IOS_VERIFIED_INTERACTIONS` against the live provider,
  with per-interaction provider-state seeding. Interactions that cannot be
  verified in the harness (Apple/FHIR upstreams, documented iOS-vs-backend
  drift) are listed in `IOS_EXCLUDED_INTERACTIONS` with reasons; the test
  fails if a contract interaction is in neither list.
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

Framework: Swift Testing

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

**Exemption — medication connect (iOS 26+):** the flow Settings → Connect
Medications → doses appear as interventions has no Maestro coverage. The
HealthKit per-object permission sheet is a system dialog Maestro cannot
drive, and the simulator has no way to seed medication dose events. Verify
manually on an iOS 26 device or simulator with medications configured in the
Health app: connect from Settings, log a dose as Taken in Health, run Sync
Now, and confirm the intervention appears. Unit tests cover the view-model
error paths (`SettingsViewModelTests`) and the sync loop
(`MedicationSyncTests`).

## Contract Tests (Cross-Service)

Consumer contracts live in `pact/contracts/`:
- `web-backend.json` -- what the web frontend expects from the API
- `ios-backend.json` -- what the iOS app expects from the API

When adding or changing an API endpoint:
1. Check if the endpoint is referenced in a contract.
2. If yes: update the contract and run `cargo test --test contract`.
3. If adding a new endpoint used by iOS or web: add it to the appropriate contract.
4. Every `ios-backend.json` interaction must also be classified in
   `backend/api/tests/contract/main.rs`: add its description to
   `IOS_VERIFIED_INTERACTIONS` (with provider-state seeding and Pact `type`
   matchers on server-generated fields) or to `IOS_EXCLUDED_INTERACTIONS`
   with a documented reason. The contract gate fails otherwise.

## Test Data

- Use the `fake` crate (Rust) for realistic test data.
- Use `rstest` for parameterized tests.
- Never hardcode UUIDs or timestamps.
- WireMock fixtures in `backend/tests/fixtures/<source>/` are recorded once from real APIs and committed.
