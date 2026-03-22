# AGENTS.md — OwnPulse

This file is the entry point for AI agents. It maps the codebase and defines parallel work boundaries. Read this, then read `CLAUDE.md`, then read the `AGENTS.md` in the specific directory you're working in.

---

## Repos

This project spans two repos:

| Repo | Purpose |
|------|---------|
| `ownpulse` | Application code — backend, web, iOS, Helm charts |
| `ownpulse-infra` | Infrastructure — OpenTofu, Ansible, nix-darwin, Sealed Secrets |

This file covers `ownpulse`. For infra work, see `ownpulse-infra/README.md`.

## Codebase Map

```
ownpulse/
├── backend/      Rust/Axum REST API — the source of truth for all data
├── web/          React/Vite frontend — all visualization, auth-walled SPA
├── ios/          Swift/SwiftUI — data pump, sync management, Phase 3b dashboard
├── db/           SQL migrations — append-only, shared by backend
├── schema/       Open data schema (JSON + Markdown) — updated with DB changes
├── pact/         Consumer-driven contract files — defines service interfaces
├── helm/         Kubernetes deployments — one chart per service
├── .github/      GitHub Actions workflows
└── docs/         Architecture documentation
```

---

## Agent Workspace Assignments

Each agent should work within one workspace. Cross-workspace changes require flagging in the PR description.

### Backend agent

**Owns:** `backend/`, `db/migrations/`, `schema/`

**Does not own:** `web/`, `ios/`, `helm/`, `.github/`

**Single test command:** `cd backend && cargo test`

**Interface to other agents:** REST API documented in `docs/api.md`. Pact provider verification in `backend/api/tests/contract/`. When you add or change an endpoint, update `docs/api.md` and run `cargo test --test contract`.

**Read first:** `backend/AGENTS.md`

---

### Web agent

**Owns:** `web/`

**Does not own:** `backend/`, `ios/`, `helm/`

**Single test command:** `cd web && npm test && npm run test:e2e`

**Interface to other agents:** Pact consumer contract in `pact/contracts/web-backend.json`. When you add API calls, update the contract. The backend agent verifies it.

**Read first:** `web/AGENTS.md`

---

### iOS agent

**Owns:** `ios/`

**Does not own:** `backend/`, `web/`, `helm/`

**Single test command:**
```bash
xcodebuild test -scheme OwnPulse \
  -destination 'platform=iOS Simulator,name=iPhone 16'
maestro test ios/maestro/flows/
```

**Interface to other agents:** Pact consumer contract in `pact/contracts/ios-backend.json`. When you add API calls, update the contract.

**Read first:** `ios/AGENTS.md`

---

### Infra agent

**Owns:** `helm/`, `.github/workflows/`

**Does not own:** application code in `backend/`, `web/`, `ios/`

**Test:** Helm lint (`helm lint helm/*/`) + dry-run (`helm upgrade --dry-run`)

**Read first:** `helm/README.md`

---

## Parallel Work — What's Safe

These areas can be worked on simultaneously without merge conflicts:

| Simultaneous work | Safe? | Notes |
|-------------------|-------|-------|
| `backend/api/src/integrations/garmin.rs` + `backend/api/src/routes/labs.rs` | ✅ | Different files |
| `web/src/components/Timeline/` + `web/src/components/Labs/` | ✅ | Different components |
| `ios/OwnPulse/Views/` + `ios/OwnPulse/Services/` | ✅ | Different files |
| Backend routes + web frontend | ✅ | Different services; Pact is the interface |
| `db/migrations/` + any application code | ⚠️ | Coordinate: migration must land before code that uses it |
| `pact/contracts/web-backend.json` from two agents | ❌ | One agent edits contracts at a time |
| `schema/open-schema.json` from two agents | ❌ | One agent edits schema at a time |

---

## PR Workflow

1. Create a feature branch: `git checkout -b feat/garmin-sync`
2. Open a draft PR immediately — CI will run on every push
3. Push frequently — use CI results as your feedback loop
4. Mark PR ready for review when CI is green and you're satisfied
5. Do not merge your own PR

### PR description must include

- What changed and why
- Any cross-workspace dependencies (e.g. "requires migration 0005 to land first")
- Any Pact contract changes
- Any schema changes

---

## Adding a New Integration

New data sources follow a consistent pattern. Steps:

1. Add OAuth token storage — the `integration_tokens` table already handles this
2. Create `backend/api/src/integrations/<source>.rs` — HTTP client with WireMock-compatible interface
3. Record real API responses to `backend/tests/fixtures/<source>/`
4. Add WireMock stub setup to `backend/tests/common/mock_servers.rs`
5. Add sync job to `backend/api/src/jobs/<source>_sync.rs`
6. Add OAuth routes to `backend/api/src/routes/auth.rs`
7. Add sync route to `backend/api/src/routes/integrations.rs`
8. Add integration tests covering: OAuth flow, sync happy path, sync error handling, token refresh
9. Add WireMock fixtures for all mocked responses
10. Update `docs/integrations.md`
11. Add source connection UI to `web/src/pages/Sources.tsx`
12. Add Playwright test for OAuth connection flow

---

## Adding a New Data Primitive

If it's user-defined (custom scale, event type, etc.):
- Do **not** add a new structured table
- Add a new `type` value to the `observations` table
- Add API validation for the new type's `value` JSONB shape
- Update `schema/open-schema.json`
- Add tests

If it's a new structured measurement with a HealthKit mapping:
- Add to `health_records` with appropriate `record_type` value
- Add HealthKit read/write mapping to iOS `HealthKitProvider`
- Add FHIR mapping if applicable
- Update `schema/open-schema.json`
- Add tests

---

## Running Everything Locally

```bash
# Start infrastructure
docker run -d -e POSTGRES_PASSWORD=dev -p 5432:5432 --name pg postgres:17

# Backend
export DATABASE_URL=postgres://postgres:dev@localhost:5432/health
cd db && sqlx migrate run
cd backend && cargo run -p api

# Web (separate terminal)
cd web && npm run dev

# iOS: open in Xcode, run on simulator

# Run all tests
cd backend && cargo test
cd web && npm test && npm run test:e2e
xcodebuild test -scheme OwnPulse -destination 'platform=iOS Simulator,name=iPhone 16'
maestro test ios/maestro/flows/
```
