# AI Agent Guide

This guide is for AI agents (Claude Code, Cursor, or similar) working on the OwnPulse codebase.

## Orientation

Read these files in order before starting any task:

1. `CLAUDE.md` (root) -- project philosophy, conventions, what not to do
2. `.claude/CLAUDE.md` -- hard rules, agent inventory, team workflow
3. `AGENTS.md` (root) -- codebase map, workspace boundaries
4. `AGENTS.md` in the specific service directory you are working in (`backend/`, `web/`, or `ios/`)

## Session Setup

Use `opdev session` to create an isolated working environment:

```bash
cd ~/src/ownpulse/ownpulse
opdev session backend-auth    # creates worktree, launches claude
```

Each session gets its own git worktree and branch. You can run multiple sessions in parallel against the same repo without conflicts.

Within a session, the lead Claude instance orchestrates work by spawning specialized agents. Write agents are spawned with `isolation: "worktree"` for further isolation.

## Before Making Changes

Run the test suite for your service area to establish a baseline:

- Backend: `cargo test`
- Web: `cd web && npm test`
- iOS: `xcodebuild test -scheme OwnPulse -destination 'platform=iOS Simulator,name=iPhone 16'`

If tests already fail before your changes, note that in the PR description.

## Working Boundaries

Each service is independently buildable and testable. Work within one service at a time.

- **Backend agent** owns `backend/`, `db/migrations/`, `schema/`
- **Web agent** owns `web/`
- **iOS agent** owns `ios/`
- **Infra agent** owns `helm/`, `.github/workflows/`

If you need to modify files outside your workspace, flag it in the PR description.

## Agent Team Workflow

For tasks touching multiple areas (backend + frontend, infra + app code):

1. Break the task into parallel units that map to available agents
2. For cross-cutting features, define the API contract first (endpoint path, request/response types, error codes)
3. Spawn write agents with `isolation: "worktree"` — each gets an isolated git copy automatically
4. Run review agents (code-review, security-review) on the results
5. Fix issues or ask agents to revise before reporting done

## Hard Rules

These are enforced in `.claude/CLAUDE.md` and violations will be reverted:

1. **Everything is IaC** — no ad-hoc kubectl, helm install, tofu apply, or SSH
2. **No telemetry or analytics** without explicit user consent
3. **No health data in logs** or error messages
4. **Never skip tests** to make CI pass
5. **Secrets in SOPS + age only**
6. **Self-hosting must work** — no required cloud services
7. **Stay in your lane** — don't modify files outside your assigned area without flagging it

## PR Workflow

1. Create a feature branch: `git checkout -b feat/description`
2. Open a **draft PR** as soon as you start significant work
3. Push frequently -- use CI results as your feedback loop
4. Mark the PR ready for review when CI is green
5. Do not merge your own PR -- humans review and merge

## What Not to Do

- Do not disable or skip tests to make CI pass
- Do not add `#[allow(dead_code)]` or `// eslint-disable` to silence warnings
- Do not hit real external APIs in tests -- use WireMock fixtures
- Do not assume a running database in tests -- use testcontainers
- Do not add analytics or telemetry
- Do not validate substance names (non-judgmental by design)
- Do not run infrastructure commands manually -- use IaC

## Interface Contracts

Services communicate via the REST API. The interface is defined by:
- `docs/architecture/api.md` -- authoritative API documentation
- `pact/contracts/web-backend.json` -- web consumer contract
- `pact/contracts/ios-backend.json` -- iOS consumer contract

When you change an API endpoint, update the docs and check if it is covered by a Pact contract. Run `cargo test --test contract` to verify.
