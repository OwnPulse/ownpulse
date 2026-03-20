# CLAUDE.md — OwnPulse

This is the root conventions file. Read it before making any changes. Then read the `AGENTS.md` in the specific directory you're working in.

---

## Project Philosophy

1. **Data sovereignty** — nothing leaves the user's instance without explicit opt-in
2. **No lock-in** — full export, always, in open formats
3. **Open schema** — versioned, public, documented
4. **Non-judgmental** — all interventions are legitimate data; never validate, filter, or warn on substance names anywhere in the stack
5. **Manual entry is first-class** — every data type supports manual entry; wearables are optional
6. **Bidirectional sync** — data written here writes back to HealthKit where a mapping exists; not a silo
7. **Federation-ready** — hooks in place, implementation deferred
8. **Tests are not optional** — every function has a unit test, every endpoint has an integration test, every flow has an E2E test; CI enforces this

**License: AGPL-3.0.** Include this header in every new source file:

```rust
// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors
```

---

## Working With AI Agents

This repo is designed for parallel AI agent work (Claude Code, Cursor). Read this section before starting any task.

### Orientation

1. Read this file (done)
2. Read `AGENTS.md` in the root — it maps the full codebase and tells you which agent owns what
3. Read the `AGENTS.md` in the specific directory you're working in
4. Run the test suite for your area before making changes — establish a baseline

### Agent boundaries

Each service is independently buildable and testable. Work within one service at a time.

| Directory | Single command to test |
|-----------|----------------------|
| `backend/` | `cargo test` |
| `web/` | `npm test` |
| `ios/` | `xcodebuild test -scheme OwnPulse` + `maestro test ios/maestro/flows/` |

### Interface boundaries

The services communicate via the REST API. The interface is defined by:
- `docs/api.md` — authoritative API documentation
- `pact/contracts/ios-backend.json` — iOS consumer contract
- `pact/contracts/web-backend.json` — web consumer contract

When you change an API endpoint, check if it's covered by a contract and update it. Run `cargo test --test contract` to verify.

### Opening PRs

- Open a draft PR as soon as you start significant work
- Push regularly — CI runs on every push, giving you feedback
- Don't merge your own PRs — humans review and merge
- One concern per PR — don't bundle unrelated changes

### What NOT to do as an agent

- Don't modify files outside your assigned service without flagging it
- Don't disable or skip tests to make CI pass — fix the underlying problem
- Don't add `#[allow(dead_code)]` or `// eslint-disable` to silence warnings — fix them
- Don't hit real external APIs in tests — use WireMock fixtures
- Don't assume a running database in tests — use testcontainers

---

## Architecture

Three services, one API. k3d on dev machines, k3s on servers — same Helm charts, same kubectl, different install method:

```
web/        React + Vite + unovis    →  app.<domain>   (nginx container)
backend/    Rust + Axum              →  api.<domain>   (Rust binary container)
ios/        Swift + SwiftUI          →  iOS device/simulator
```

All three consume the same REST API with JWT auth. The web and iOS share the same auth flow and token format.

---

## Repository Layout

```
ownpulse/
├── CLAUDE.md                   # This file
├── AGENTS.md                   # Agent workspace map (read this next)
├── ios/
│   ├── AGENTS.md
│   ├── OwnPulse/
│   ├── OwnPulseTests/ # Swift Testing
│   ├── OwnPulseUITests/
│   └── maestro/flows/          # Maestro E2E
├── web/
│   ├── AGENTS.md
│   ├── src/
│   ├── tests/unit/             # Vitest
│   └── tests/e2e/              # Playwright
├── backend/
│   ├── AGENTS.md
│   ├── Cargo.toml
│   └── api/
│       ├── src/
│       │   ├── auth/
│       │   ├── routes/
│       │   ├── models/
│       │   ├── db/
│       │   ├── jobs/
│       │   ├── integrations/   # One module per data source
│       │   ├── export/
│       │   ├── stats.rs        # Correlation math (Phase 3)
│       │   └── crypto.rs
│       └── tests/
│           ├── common/         # Testcontainers setup, helpers
│           ├── unit/
│           ├── integration/
│           └── contract/       # Pact provider verification
├── pact/contracts/             # Committed Pact contract files
├── db/migrations/              # Append-only SQL migrations
├── schema/
│   ├── open-schema.json        # Canonical versioned export schema
│   └── open-schema.md
├── helm/
│   ├── api/
│   ├── web/
│   ├── postgres/
│   ├── arc/                    # Actions Runner Controller
│   └── woodpecker/
├── .github/workflows/
│   ├── backend.yml
│   ├── web.yml
│   ├── ios.yml
│   └── deploy.yml
└── docs/
    ├── README.md               # Docs index
    ├── architecture/           # System design, data model, API reference, ADRs
    ├── decisions/              # Architecture Decision Records
    ├── design/                 # Brand, wireframes, design system
    ├── guides/                 # Contributing, agent guide, testing, self-hosting
    ├── cooperative/            # Governance, data sharing, privacy principles
    └── legal/                  # Privacy policy and ToS drafts
```

---

## Backend Conventions (Rust)

### Toolchain and style

- Rust stable, edition 2021
- Axum 0.7, Tokio full runtime, SQLx 0.7
- `sqlx::query_as!` macros throughout — compile-time query checking
- `thiserror` for library error types, `anyhow` acceptable in binary entry points only
- `tracing` for all logging — no `println!`; JSON in production, pretty in dev
- `cargo clippy -- -D warnings` — no warnings on `main`; CI enforces this

### Module structure

```
auth/           JWT issue/verify, refresh token logic, middleware extractor
routes/         One file per route group; each file is a function returning a Router
models/         Serde structs for request/response; separate from DB types
db/             SQLx query functions; no business logic
jobs/           Tokio background tasks; one file per integration sync job
integrations/   HTTP clients for external APIs; WireMock-compatible
export/         Streaming export logic; never buffers full dataset
stats.rs        Pearson correlation, lag correlation, rolling averages
crypto.rs       AES-256-GCM; all token encrypt/decrypt goes through here
```

### Testing (backend)

**Unit tests** — in `tests/unit/` or as `#[cfg(test)]` modules:
- No database, no network
- Pure functions: stats math, crypto operations, data transformations, route handlers with mocked DB
- `cargo test --lib`

**Integration tests** — in `tests/integration/`:
- `testcontainers-rs` — every test module calls `common::setup_db()` which spins up an ephemeral Postgres container
- External APIs mocked with `wiremock` — fixtures in `tests/fixtures/<source>/`
- Tests are fully parallel-safe — no shared state
- `cargo test --test integration`

**Contract tests** — in `tests/contract/`:
- `pact_verifier` reads `pact/contracts/*.json`
- Spins up the API against a testcontainers Postgres
- `cargo test --test contract`

**Test data generation:**
- Use `fake` crate for realistic test data
- Use `rstest` for parameterized tests
- Never hardcode UUIDs or timestamps in tests

### HealthKit write-back (unconditional rule)

Records with `source = 'healthkit'` are **never** inserted into `healthkit_write_queue`. This check happens in the service layer, not the route handler. It is not configurable. It cannot be bypassed by any API parameter.

### Deduplication

Before inserting any `health_record`, query for existing records within 60 seconds and 2% value tolerance from a different source. If found: log a structured warning with both record IDs and sources, apply `source_preferences` for this metric type, insert the record with a `duplicate_of` reference (add this nullable column). Never silently drop.

---

## Schema Conventions

- Postgres 16
- All migrations in `db/migrations/`, numbered: `0001_init.sql`
- **Never edit existing migrations** — add new ones
- All PKs: `UUID`, `gen_random_uuid()`
- All timestamps: `TIMESTAMPTZ`
- Schema changes require updating `schema/open-schema.json` and `docs/data-model.md`
- Run `cargo sqlx prepare` after any schema or query change; commit `.sqlx/`

### Key tables

| Table | Type | Purpose |
|-------|------|---------|
| `users` | structured | Accounts |
| `health_records` | structured | All wearable/device measurements |
| `interventions` | structured | Substances, meds, supplements — no name validation |
| `daily_checkins` | structured | Five 1-10 subjective scores |
| `lab_results` | structured | Blood panel data |
| `calendar_days` | structured | Meeting aggregates |
| `genetic_records` | structured | SNP variants, stored verbatim |
| `observations` | **flexible** | All user-defined: events, scales, symptoms, notes, context tags, environmental |
| `source_preferences` | structured | Per-metric source-of-truth preference |
| `healthkit_write_queue` | structured | Pending HealthKit write-backs |
| `integration_tokens` | structured | OAuth tokens for all integrations (AES-256-GCM encrypted) |
| `refresh_tokens` | structured | JWT refresh tokens |
| `sharing_consents` | structured | Cooperative data sharing consent |
| `export_jobs` | structured | Export audit log |

### `observations` table

The extensibility layer. `type` is one of: `event_instant`, `event_duration`, `scale`, `symptom`, `note`, `context_tag`, `environmental`. `value` is JSONB — validated by `type` in the API layer, not at DB level. **Do not add new structured tables for new user-defined data types** — add a new `type` value to `observations` instead.

`value` JSONB by type:
```
event_instant:     {} or {"notes": "15 min at 90°C"}
event_duration:    {} or {"notes": "..."}
scale:             {"numeric": 6, "max": 10}
symptom:           {"severity": 4}
note:              {"text": "..."}
context_tag:       {}
environmental:     {"numeric": 22.5, "unit": "celsius"}
```

### Cooperative data boundary

`sharing_consents` is the trust boundary. **Never** aggregate another user's data without checking for active consent. Genetic data requires a separate consent record with `dataset = 'genetics'` — this is independent from and stricter than health data consent. Consent revocation takes effect immediately — no grace period.

---

## Testing Conventions

### The rule

Every PR must maintain or improve coverage. CI fails if:
- Any test fails
- `cargo clippy -- -D warnings` has warnings
- `cargo sqlx prepare --check` is stale
- `tsc --noEmit` has errors
- Playwright E2E fails

### Backend

```bash
cargo test --lib                   # unit tests
cargo test --test integration      # integration (testcontainers)
cargo test --test contract         # pact verification
cargo test                         # all of the above
```

### Web

```bash
npm test                           # vitest (unit + component)
npm run test:e2e                   # playwright
npm run type-check                 # tsc --noEmit
```

### iOS

```bash
xcodebuild test \
  -scheme OwnPulse \
  -destination 'platform=iOS Simulator,name=iPhone 16'

maestro test ios/maestro/flows/    # E2E flows
```

### WireMock fixtures

External API responses are recorded once and committed to `backend/tests/fixtures/<source>/`. Structure:

```
tests/fixtures/
├── garmin/
│   ├── activities-list.json
│   └── hrv-summary.json
├── oura/
│   ├── readiness.json
│   └── sleep.json
└── dexcom/
    └── egvs.json
```

WireMock stubs are set up in `tests/common/mock_servers.rs`. Never modify fixtures to make a test pass — if the API response format changes, update the fixture and the parsing logic together.

### Pact contracts

Consumer contracts live in `pact/contracts/`. When adding or changing an endpoint:
1. Check if the endpoint is referenced in any contract file
2. If yes: update the contract, run `cargo test --test contract` to verify backend still satisfies it
3. If adding a new endpoint used by iOS or web: add it to the appropriate consumer contract

---

## Web Frontend Conventions

- React 18, TypeScript strict mode, Vite
- **Charts: unovis only** (`@unovis/ts` + `@unovis/react`) — do not use recharts, Chart.js, or D3 directly
- State: Zustand for client state, TanStack Query for server state + caching
- API calls: typed wrappers in `web/src/api/` — never raw fetch in components
- Auth: JWT in memory + httpOnly cookie for refresh token — **never localStorage**
- MSW (Mock Service Worker) for unit test API mocking
- Playwright for E2E — tests run against real backend via testcontainers (Postgres) + backend process in CI

---

## iOS Conventions

- Swift 6, SwiftUI, iOS 18 minimum
- **No third-party dependencies except GRDB** (offline queue) and Swift Testing
- Charts: **Swift Charts** (native, Phase 3b+) — no third-party charting library
- Unit tests: **Swift Testing** framework (Xcode 16) — not XCTest
- UI tests: **XCUITest** for complex flows, **Maestro** for E2E
- SwiftUI testing: **ViewInspector** for testing views without simulator
- HealthKit abstracted behind `HealthKitProvider` protocol — `MockHealthKitProvider` in tests
- Network abstracted behind `NetworkClient` protocol — mock in tests
- JWT in **Keychain** only — never `UserDefaults`
- Offline queue: GRDB SQLite — failed syncs retry on next foreground/background refresh
- Accessibility identifiers on all interactive elements — XCUITest uses these, never text matching
- Phase 1: no charts — "Open Dashboard" button opens `app.<domain>` in Safari
- Phase 3b: Swift Charts for dashboard — hero metric, sparklines, today card, weekly summary

### Maestro flows

Flows live in `ios/maestro/flows/`. File names match the user story: `log-intervention.yaml`, `complete-checkin.yaml`, `connect-garmin.yaml`. Flows must be deterministic — use `assertVisible` to confirm state before acting.

---

## CI/CD

### Runner strategy

**Linux jobs** (backend, web): ARC (Actions Runner Controller) — ephemeral runner pods in k3s cluster on the droplet, autoscale to zero, label `arc-runner-set`.

**macOS/iOS jobs**: Self-hosted Mac mini M4 + Tart VMs. Each job gets a fresh ephemeral macOS VM with Xcode 16 pinned. Label `macos-tart`. No GitHub-hosted macOS runners — they are slow, flaky, and expensive. See `docs/infrastructure.md` for full Mac mini setup runbook.

**Connectivity**: Tailscale connects Mac mini, droplet, and dev machines. No public ports on the Mac mini.

```
backend.yml    unit + integration + contract + clippy + sqlx check + build   runs-on: arc-runner-set
web.yml        vitest + playwright + tsc + build                             runs-on: arc-runner-set
ios.yml        swift testing + xcuitest + maestro (inside Tart VM)          runs-on: macos-tart
deploy.yml     helm upgrade when backend + web pass on main                  runs-on: arc-runner-set
```

All jobs idempotent. No shared runner state. iOS failures do not block deploy — pragmatic for solo dev, tighten when team grows.

---

## Environment Variables

| Variable | Required | Notes |
|----------|----------|-------|
| `DATABASE_URL` | yes | Postgres connection string |
| `JWT_SECRET` | yes | Min 32 bytes — `openssl rand -hex 32` |
| `JWT_EXPIRY_SECONDS` | no | Default 3600 |
| `REFRESH_TOKEN_EXPIRY_SECONDS` | no | Default 2592000 |
| `GOOGLE_CLIENT_ID` | yes | |
| `GOOGLE_CLIENT_SECRET` | yes | |
| `GOOGLE_REDIRECT_URI` | yes | |
| `GARMIN_CLIENT_ID` | yes | |
| `GARMIN_CLIENT_SECRET` | yes | |
| `OURA_CLIENT_ID` | yes | |
| `OURA_CLIENT_SECRET` | yes | |
| `DEXCOM_CLIENT_ID` | yes | Phase 2 |
| `DEXCOM_CLIENT_SECRET` | yes | Phase 2 |
| `ENCRYPTION_KEY` | yes | 32-byte hex for AES-GCM |
| `STORAGE_PATH` | yes | Local path or S3-compatible URL |
| `APP_USER` | yes | Single username for personal instance |
| `APP_PASSWORD_HASH` | yes | bcrypt hash |
| `DATA_REGION` | no | `us` or `eu`, default `us` |
| `WEB_ORIGIN` | yes | CORS allowlist for web frontend URL |
| `RUST_LOG` | no | Default `info` |

---

## What Not To Do

- **Don't validate substance names.** The platform is non-judgmental by design.
- **Don't write healthkit-sourced records back to HealthKit.** The cycle guard is unconditional.
- **Don't silently drop duplicate records.** Log the conflict, apply source_preferences, preserve provenance.
- **Don't add new structured tables for user-defined data types.** Add a new `type` to `observations`.
- **Don't buffer export responses.** Stream everything.
- **Don't skip or disable tests.** Fix the underlying problem.
- **Don't hit real external APIs in tests.** Use WireMock fixtures.
- **Don't aggregate another user's data without checking sharing_consents.**
- **Don't include genetic data in cooperative aggregates without `dataset = 'genetics'` consent.**
- **Don't interpret genetic records.** Store verbatim; annotation is Phase 4.
- **Don't implement federation.** Leave `federation_id` and `source_instance` nullable with no logic.
- **Don't add analytics or telemetry.**
- **Don't store JWT in iOS UserDefaults or web localStorage.**
- **Don't use recharts, Chart.js, or raw D3 in the web frontend.** Use unovis.
- **Don't use third-party charting libraries in the iOS app.** Use Swift Charts.

---

## Local Setup

**k3d vs k3s:** k3d runs k3s inside Docker on your dev machine. k3s runs directly on Linux servers (droplet, self-hosted VPS). Same Helm charts, same kubectl commands — only the install method differs. Use k3d locally, k3s in production.

### Option A: k3d (full local cluster — matches production exactly)

```bash
# Create local cluster
k3d cluster create ownpulse-local --port "8080:80@loadbalancer"

# Deploy all services via Helm (same as production)
helm upgrade --install postgres helm/postgres -n ownpulse --create-namespace
helm upgrade --install api helm/api -n ownpulse
helm upgrade --install web helm/web -n ownpulse

# Run migrations
export DATABASE_URL=postgres://postgres:dev@localhost:5432/ownpulse
cd db && sqlx migrate run
```

### Option B: services only (faster iteration)

```bash
# Postgres only via Docker (no full cluster)
docker run -d -e POSTGRES_PASSWORD=dev -p 5432:5432 --name pg postgres:16
export DATABASE_URL=postgres://postgres:dev@localhost:5432/ownpulse
cd db && sqlx migrate run
cd backend && cargo sqlx prepare --workspace
cargo run -p api         # API on :8080

# Web
cd web && npm install && npm run dev   # Web on :5173

# iOS
open ios/OwnPulse.xcodeproj           # Point to http://localhost:8080
```

Option B is faster for day-to-day backend and web development. Use Option A when testing Helm chart changes, ingress config, or anything infrastructure-related.

### Running tests

```bash
# Backend (testcontainers handles Postgres — no running DB needed)
cargo test

# Web
cd web && npm test && npm run test:e2e

# iOS
xcodebuild test -scheme OwnPulse \
  -destination 'platform=iOS Simulator,name=iPhone 16'
maestro test ios/maestro/flows/
```

### Developer API credentials

Register early — some have approval delays:
- **Garmin Connect API** — developer.garmin.com — human review, can take 1-2 weeks
- **Oura API** — cloud.ouraring.com/personal-access-tokens — personal token, instant
- **Dexcom Developer** — developer.dexcom.com — approval required, a few days
- **Google Cloud Console** — instant
