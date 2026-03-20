# ADR-0009: Testing Strategy (Testcontainers, WireMock, Pact, Playwright, Maestro)

**Date:** 2026-03-17
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

OwnPulse is designed for parallel AI agent development. Multiple agents (Claude Code, Cursor) may work on different parts of the codebase simultaneously. Without a comprehensive, automated test suite, agents have no reliable signal about whether their changes are correct or whether they've broken something another agent was relying on.

The test suite must:
- Run automatically on every pull request
- Give agents actionable failure messages (not just "tests failed")
- Be hermetic — no test should depend on another test's side effects
- Cover unit logic, API endpoints, service integrations, and user-visible flows
- Define the interface between services (iOS ↔ backend, web ↔ backend) so agents can work independently
- Run fast enough that CI feedback is useful (under 10 minutes for any single service)

Additionally, the platform integrates with external APIs (Garmin, Oura, Dexcom, Google) that cannot be called in CI. Tests that hit real external APIs are slow, flaky, and quota-consuming.

---

## Decision

Use a layered testing strategy:

**Backend (Rust):**
- Unit tests: `#[cfg(test)]` modules and `tests/unit/` — pure functions, no DB, no network
- Integration tests: `tests/integration/` — testcontainers for ephemeral Postgres, WireMock for external APIs
- Contract tests: `tests/contract/` — Pact provider verification against committed consumer contracts

**Web frontend (React):**
- Unit/component tests: Vitest + React Testing Library + MSW (Mock Service Worker)
- E2E tests: Playwright against real backend (Docker Compose in CI)

**iOS (Swift):**
- Unit tests: Swift Testing framework (parallel by default) + ViewInspector for SwiftUI
- UI tests: XCUITest for complex flows
- E2E tests: Maestro (YAML-based flows, readable by AI agents)

**Contract layer (cross-service):**
- Pact consumer contracts: iOS and web publish contracts to `pact/contracts/` in git
- Backend verifies both contracts on every PR

---

## Alternatives Considered

### Shared test database (not ephemeral)

Run all integration tests against a persistent test database. Faster setup than testcontainers (no container startup per run).

Rejected because:
- Tests can interfere with each other via shared state.
- Test order matters — a test that runs after a dirty test may fail or produce incorrect results.
- Parallel test execution (which Rust enables) requires isolated databases.
- Testcontainers startup overhead (~2-3 seconds) is acceptable for the reliability guarantee it provides.

### HTTP record-and-replay (vcr/cassettes) instead of WireMock

Record real API responses once, replay them in tests. Used by Ruby's VCR gem and Python's responses library.

Rejected because:
- Less control over response scenarios — hard to test error cases, rate limiting, token expiry.
- Cassette files can grow large and are harder to read/edit than WireMock stub JSON.
- WireMock allows programmatic setup of scenarios (success, 429, 500, token refresh) in the test setup code.

### Skip contract tests, use shared API client library

Generate an API client from an OpenAPI spec and share it between iOS, web, and tests.

Considered as a complement. Rejected as a replacement for Pact because:
- A shared client only tests that the client and server agree on the schema, not on behavior.
- Pact tests specific interactions with specific request/response pairs — it catches regressions in business logic, not just schema changes.
- Pact contracts are committed to git — they serve as executable documentation of what each consumer expects.
- OpenAPI generation can be added later without replacing Pact.

### Cypress instead of Playwright (web E2E)

Cypress is widely used and has good developer experience.

Rejected because:
- Playwright runs tests in parallel by default; Cypress is single-threaded without additional configuration.
- Playwright handles multi-tab and cross-origin flows (needed for OAuth) more cleanly.
- Playwright's trace viewer provides better debugging artifacts for CI failures.
- Playwright is now the dominant choice for new projects.

### XCUITest only for iOS E2E (no Maestro)

XCUITest is Apple's official UI testing framework. Well-supported, mature.

Rejected as the only E2E tool because:
- XCUITest requires writing Swift code — fine for human developers but harder for AI agents to write and read.
- Maestro's YAML syntax is more readable and closer to natural language: `tapOn: "Log Intervention"` vs. XCUITest's verbose element queries.
- Maestro flows serve as living documentation of user journeys.
- Both can coexist — XCUITest for complex flows that need Swift-level control, Maestro for E2E journeys.

### No contract tests (rely on integration tests only)

Run the backend integration tests against real iOS and web clients.

Rejected because:
- Real clients cannot be run in backend CI.
- Without contracts, agents working on iOS or web cannot verify their API usage is correct without deploying and running the backend.
- Contracts give agents a local, fast verification step — "does my API call match what the backend expects?" answered in seconds.

---

## Consequences

**Positive:**
- Tests are fully hermetic — each integration test run gets a fresh Postgres instance, no shared state.
- External API failures or quota limits never affect CI.
- Pact contracts let iOS and web agents verify API compatibility without running the backend.
- Maestro flows are readable by AI agents — Claude Code can write and understand Maestro YAML without deep iOS knowledge.
- Playwright trace viewer uploads on failure give agents a visual record of what went wrong.
- The layered approach catches different failure modes at each layer: logic errors in unit tests, integration errors in integration tests, UX errors in E2E tests.

**Negative / tradeoffs:**
- More tooling to learn: testcontainers, WireMock, Pact, Playwright, Maestro. Each has its own documentation and configuration.
- Testcontainers requires Docker in the CI environment. ARC Linux runners have Docker available; this is not a problem but adds a dependency.
- Pact contracts must be kept up to date when APIs change. Stale contracts cause false failures. Mitigate with a clear process: API changes → update contract → verify.
- Maestro is a younger tool than XCUITest. Less Stack Overflow coverage, smaller community.

**Risks:**
- WireMock fixture files diverge from real API responses over time. Mitigate with periodic re-recording of fixtures against real APIs (development only, not CI).
- Playwright E2E tests that require a running backend are slower than unit tests. Mitigate by keeping E2E tests focused on critical user journeys only — not a replacement for unit and integration tests.
- Contract tests give false confidence if consumers don't actually use the contracts they publish. Enforce by making contract generation a required step in consumer CI.

---

## References

- testcontainers-rs: https://github.com/testcontainers/testcontainers-rs
- wiremock-rs: https://github.com/LukeMathWalker/wiremock-rs
- Pact foundation: https://pact.io
- Playwright: https://playwright.dev
- Maestro: https://maestro.mobile.dev
- Swift Testing: https://developer.apple.com/documentation/testing
- ViewInspector: https://github.com/nalexn/ViewInspector
