# OwnPulse Documentation

This directory contains all project documentation. Start with the section relevant to your task.

## Sections

| Directory | Contents |
|-----------|----------|
| [architecture/](architecture/) | System architecture overview, data model, API reference, HealthKit sync design. |
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
