# ADR-0004: React + Vite + unovis for Web Frontend

**Date:** 2026-03-17
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

OwnPulse's web frontend is the primary data exploration surface — the timeline overlay, correlation explorer, lab result trends, and intervention reports all live here. The charting requirements are specific: multi-metric time-series with different y-axes per metric, event markers on a shared time axis, scatter plots for correlation, and sparklines. These are more demanding than what a generic UI framework handles well out of the box.

The frontend must be:
- An auth-walled SPA (single page app) served by nginx
- Independently deployable from the backend (separate Helm chart)
- Testable with Vitest (unit/component) and Playwright (E2E)
- Navigable by AI agents without deep framework expertise

The founder has prior React experience, which reduces the onboarding cost of React relative to alternatives.

---

## Decision

Use **React 18** with **TypeScript** (strict mode), **Vite** as the bundler, and **unovis** as the charting library. State management via **Zustand** (client state) and **TanStack Query** (server state + caching).

---

## Alternatives Considered

### SvelteKit

SvelteKit has genuine technical advantages for a reactive dashboard: less boilerplate, faster runtime by default, no virtual DOM overhead, and an elegant reactive primitive model. For a greenfield project without framework familiarity constraints, SvelteKit would be a strong choice.

Rejected because:
- The founder has React experience and none with Svelte. The learning curve has a real cost during early development.
- The AI agent ecosystem (Claude Code, Cursor) has substantially more React training data than Svelte. Agents produce higher-quality React code.
- Vitest and Playwright both work with Svelte, but the Testing Library ecosystem is richer for React.
- The unovis library has first-class React wrappers; Svelte support is less mature.

SvelteKit remains worth reconsidering if the frontend needs a significant rewrite and Svelte expertise is available.

### Vue 3 + Vite

Vue 3 with the Composition API is close to React in developer experience. Good TypeScript support, Vite as standard bundler.

Rejected because:
- Similar reasoning to Svelte — less framework familiarity, smaller AI agent training corpus for Vue vs React.
- The unovis React wrappers are more mature than Vue equivalents.

### Next.js (React meta-framework)

Next.js adds SSR, SSG, and a file-based router on top of React. Widely used, excellent tooling.

Rejected because:
- OwnPulse's web frontend is an auth-walled SPA — there are no public pages that benefit from SSR or SSG.
- Next.js adds deployment complexity (Node.js server or edge runtime) that conflicts with the nginx-served static build model.
- Vite is faster for development iteration and produces a clean static build.

### Recharts or Chart.js (charting)

Both are popular React-compatible charting libraries with large communities.

Rejected in favor of unovis because:
- The multi-metric overlay pattern (different y-axes per metric, shared time axis, event markers) is a first-class use case in unovis and requires significant workarounds in recharts or Chart.js.
- unovis is designed for data-dense dashboards — it handles the density of health time-series data more gracefully.
- unovis is TypeScript-first with excellent type inference.
- Chart.js has a non-React rendering model that creates awkward integration patterns in React applications.

### D3.js directly

D3 gives maximum control and is the foundation for most charting libraries.

Rejected for direct use because:
- D3's imperative DOM manipulation style conflicts with React's declarative rendering model — integrating them correctly requires significant boilerplate.
- The learning curve is steep and would slow initial development significantly.
- unovis provides D3-quality output with a React-native API.

---

## Consequences

**Positive:**
- React familiarity reduces early development friction.
- Strong AI agent support — agents produce idiomatic React code consistently.
- Vite's hot module replacement is fast; development iteration is quick.
- TanStack Query handles caching, background refetch, and optimistic updates — reduces the amount of data fetching logic to write.
- Zustand is minimal and readable — easy for agents to reason about.
- unovis handles the multi-metric timeline out of the box.
- Playwright + React Testing Library is a mature, well-documented testing stack.

**Negative / tradeoffs:**
- React's bundle size is larger than Svelte's compiled output. Acceptable for an auth-walled SPA where the first load is a one-time cost per session.
- unovis is less widely known than recharts or Chart.js — fewer Stack Overflow answers, smaller community.
- TypeScript strict mode catches real bugs but slows initial development for complex types.

**Risks:**
- unovis is a younger library than recharts or Chart.js. Risk of abandonment or breaking API changes. Mitigate by pinning the version and monitoring the repo actively.
- React 18 concurrent features (Suspense, transitions) add complexity if used carelessly. Stick to standard patterns; use concurrent features only where specifically beneficial.

---

## References

- unovis documentation: https://unovis.dev
- Vite documentation: https://vitejs.dev
- TanStack Query: https://tanstack.com/query
- Zustand: https://github.com/pmndrs/zustand
- React Testing Library: https://testing-library.com/docs/react-testing-library/intro
