# Web Frontend — AGENTS.md

> **Note:** Always work in a git worktree, not the primary checkout. See root CLAUDE.md § "Git worktrees".

## Ownership

**Owns:** `web/`

**Does NOT own:** `backend/`, `ios/`, `helm/`

## Build

```
npm run build
```

## Test

```
npm test && npm run test:e2e
```

## Interface to Other Services

- Pact consumer contract at `pact/contracts/web-backend.json`

## Common Patterns

- React 18 with TypeScript in strict mode
- Vite as the build tool / dev server
- Charts rendered with unovis
- Client state managed with Zustand
- Server state managed with TanStack Query
- All API calls go through typed wrappers in `src/api/`
- JWT kept in memory only (never persisted to storage)

## What NOT to Do

- **No recharts, Chart.js, or D3** — use unovis for all charting.
- **No storing JWT in `localStorage`** (or `sessionStorage`) — keep it in memory.
- **No raw `fetch` calls inside components** — use the typed API wrappers in `src/api/`.
- **No analytics or telemetry** — do not add tracking scripts, pixels, or telemetry SDKs.
