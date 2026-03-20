# ADR-0011: Separate Repository for Public Website (ownpulse-web)

**Date:** 2026-03-18
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

OwnPulse needs a public-facing website at `ownpulse.health` — a landing page, about page, and eventually docs and blog. This site serves a fundamentally different purpose from the application (the backend API, React dashboard, and iOS app in `ownpulse/ownpulse`).

The public site is marketing and communication infrastructure. The application is product. They have different audiences, different deployment cadences, different technology requirements, and different contributor profiles. Coupling them creates unnecessary friction in both directions.

Key forces:

- **The public site is static HTML.** It needs no backend, no database, no authentication. Astro generates pure HTML at build time. The application is a full-stack SPA with a Rust API and Postgres.
- **Self-hosters don't need it.** Someone running OwnPulse on their own VPS has no reason to deploy the public website. Including it in the application repo creates confusion about what's required.
- **Deployment cadence differs.** The public site may change weekly (copy edits, blog posts, waitlist updates). The application deploys on feature completion. Coupling them means the site can't ship without passing application CI.
- **CI is different.** The application repo runs `cargo test`, `npm test`, `playwright`, `xcodebuild`, and `maestro`. The public site needs `astro build` — a 500ms static build. Running the full application CI for a copy edit is wasteful.
- **Contributors differ.** A designer or writer updating the about page shouldn't need Rust, Xcode, or an understanding of the application's test infrastructure.

---

## Decision

Maintain the public website in a **separate repository**: `ownpulse/ownpulse-web`.

**Stack:** Astro 4 + Tailwind CSS. Static output only — `astro build` produces pure HTML/CSS/JS in `dist/`. No JavaScript framework. The only client-side JS is a vanilla fetch for the waitlist form.

**Deployment:** nginx container in the k3s cluster, served at `ownpulse.health`. Own Helm chart (`helm/web-public/`), own Dockerfile (multi-stage: node builder → nginx:alpine), own nginx.conf.

**Relationship to the application repo:**
- The public site links to the application's GitHub repo, self-hosting guide, and open data schema.
- The public site uses the same brand system (colors, typography, voice) defined in `docs/design/brand.md` in the application repo.
- There is no code dependency between them. No shared packages, no monorepo tooling, no workspace linking.

**What lives where:**

| Content | Repo |
|---------|------|
| Landing page, about page, blog (future) | `ownpulse-web` |
| React dashboard (app.ownpulse.health) | `ownpulse` → `web/` |
| Rust API (api.ownpulse.health) | `ownpulse` → `backend/` |
| iOS app | `ownpulse` → `ios/` |
| Brand system (design tokens, voice) | `ownpulse` → `docs/design/brand.md` (source of truth, duplicated into Tailwind config in ownpulse-web) |

---

## Alternatives Considered

### Public site inside the application monorepo

Add a `site/` or `public/` directory to `ownpulse/ownpulse`. Keep everything in one repo.

Pros: single repo to clone, one set of branch protection rules, brand tokens can be shared as a build step.

Rejected because:
- CI coupling: every site change triggers the full application CI pipeline (Rust compile, integration tests, iOS build). The site build takes 500ms; waiting for `cargo test` and `xcodebuild` adds minutes.
- Deployment coupling: site and application share a deploy trigger. A copy fix on the landing page shouldn't require passing `cargo clippy -- -D warnings`.
- Conceptual confusion: self-hosters see the public site in the repo and wonder if they need to deploy it. The `helm/` directory already has `api/` and `web/` — adding `site/` muddies which Helm charts are required.
- Contributor friction: updating the about page requires cloning a repo with Rust, Node, and Swift dependencies, even though none of them are relevant.

### Static hosting service (Netlify, Vercel, Cloudflare Pages)

Deploy the static site to a managed platform instead of self-hosting in k3s.

Pros: zero infrastructure to manage, automatic SSL, CDN, preview deploys on PRs.

Rejected because:
- Adds a third-party dependency for a critical piece of infrastructure (the project's public face).
- The k3s cluster already runs nginx for the application frontend — adding one more static site is trivial.
- Keeps deployment consistent: everything runs in the same cluster, deployed via the same `helm upgrade` pattern.
- Cost: Cloudflare Pages is free, but the consistency argument outweighs the convenience.

Worth reconsidering if the site grows to include a blog with frequent updates and preview deploys become valuable.

### GitHub Pages

Serve the static build from GitHub Pages on a custom domain.

Rejected for similar reasons as managed hosting — it's a third-party dependency for something the cluster already handles. Also, GitHub Pages doesn't support custom server-side behavior (like the nginx stub for `/api/waitlist`).

---

## Consequences

**Positive:**
- The public site ships independently. Copy changes, design updates, and blog posts don't touch the application CI pipeline.
- Self-hosters see a clean application repo with only the components they need to deploy.
- Contributors can update the site with only Node.js installed — no Rust, no Xcode.
- The Astro build is fast (~500ms). CI for the public site is a single `npm run build` check.
- Deployment is consistent with the rest of the platform: nginx container, Helm chart, same k3s cluster.

**Negative / tradeoffs:**
- Brand tokens (colors, fonts, type scale) are duplicated between `docs/design/brand.md` and `tailwind.config.mjs` in ownpulse-web. Changes to the brand system require updating both. This is a manual process.
- Two repos to manage: branch protection, CI workflows, and dependency updates are configured separately.
- New contributors must know which repo to clone for which type of change.

**Risks:**
- Brand drift: the public site's Tailwind config diverges from the application's design tokens over time. Mitigate by treating `docs/design/brand.md` as the source of truth and reviewing the public site's config when brand.md changes.
- The public site grows complex enough (docs, blog, interactive demos) that Astro's static model becomes limiting. Mitigate by keeping the site static as long as possible — Astro handles content collections and MDX well, which covers docs and blog without a framework.

---

## References

- Astro documentation: https://astro.build
- Tailwind CSS: https://tailwindcss.com
- ADR-0006 (k3s deployment model): applicable to the public site's Helm chart
- `docs/design/brand.md` in `ownpulse/ownpulse`: canonical brand system
