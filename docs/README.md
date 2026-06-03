# OwnPulse Documentation

This directory contains all project documentation. Start with the section relevant to your task.

OwnPulse separates the reviewable application from the hosted-service operation. The `ownpulse`
repository contains the open application, schema, and self-hosting charts. The public site lives in
`ownpulse-web`, production infrastructure lives in the private `ownpulse-infra` repository, developer
workflow tooling lives in `ownpulse-dev`, and shared CI/CD actions live in `gh-actions`.

The business model is paid hosting and storage for users who want OwnPulse operated for them. Product
docs should preserve that distinction: users must be able to inspect and self-host the app, while the
hosted service earns revenue by providing reliable operations, storage, backups, and maintenance.

## Sections

| Directory | Contents |
|-----------|----------|
| [product/](product/) | Target users, use cases, and user journeys. Read this to understand who we're building for. |
| [architecture/](architecture/) | System architecture overview, data model, API reference, HealthKit sync design, hosted-service boundary. |
| [decisions/](decisions/) | Architecture Decision Records (ADRs). Numbered, append-only. Read these to understand why things are the way they are. |
| [design/](design/) | Brand guidelines, wireframes, and design system. |
| [guides/](guides/) | Contributing, agent guide, testing strategy, self-hosting, local development, integrations. |
| [cooperative/](cooperative/) | Governance model, data sharing mechanics, privacy principles. |
| [legal/](legal/) | Draft privacy policy and terms of service (require legal review before use). |

## Conventions

- Documentation files use Markdown.
- ADRs are numbered sequentially and never deleted or renumbered.
- When code changes affect documented behavior, update the relevant docs in the same PR.
- The canonical API reference is `architecture/api.md`. Pact contracts in `pact/contracts/` are the executable version.
