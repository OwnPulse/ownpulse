# Contributing to OwnPulse

OwnPulse is an open source cooperative. Contributions are welcome from humans and AI agents alike.

## License

OwnPulse is licensed under AGPL-3.0. By contributing, you agree that your contributions will be licensed under the same terms.

Every new source file must include the AGPL license header:

```rust
// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors
```

For TypeScript/JavaScript:

```typescript
// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors
```

For Swift:

```swift
// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors
```

## Workflow

1. **Fork** the repository.
2. **Create a branch** from `main`: `git checkout -b feat/your-feature` or `fix/your-fix`.
3. **Read the conventions** before writing code:
   - `CLAUDE.md` -- project-wide conventions
   - `AGENTS.md` -- workspace boundaries and codebase map
   - The `AGENTS.md` in the specific service directory you are working in
4. **Make your changes.** Stay within one service per PR.
5. **Run tests** for your service before pushing:
   - Backend: `cargo test`
   - Web: `npm test && npm run test:e2e`
   - iOS: `xcodebuild test -scheme OwnPulse` + `maestro test ios/maestro/flows/`
6. **Push** and open a pull request against `main`.
7. **Describe your changes** in the PR: what changed, why, and any cross-service dependencies.
8. Wait for CI to pass and a human reviewer to approve.

## PR Guidelines

- One concern per PR. Do not bundle unrelated changes.
- Every PR must maintain or improve test coverage.
- Do not disable or skip tests to make CI pass.
- If you change an API endpoint, update `docs/architecture/api.md` and check Pact contracts.
- If you change the database schema, update `schema/open-schema.json` and `docs/architecture/data-model.md`.

## Code Style

- **Rust:** `cargo fmt --check` and `cargo clippy -- -D warnings` must pass. Use `thiserror` for errors, `tracing` for logging.
- **TypeScript:** strict mode. No `any` types.
- **Swift:** Swift 6 concurrency. No third-party dependencies except GRDB.

## Developer Tooling

### Pre-commit hooks (optional but recommended)

Install the [pre-commit](https://pre-commit.com/) framework, then enable hooks:

```bash
pip install pre-commit   # or: brew install pre-commit
pre-commit install
```

This runs formatting and lint checks automatically before each commit:
- `cargo fmt --check` for Rust files
- `biome check` for web frontend files
- `tsc --noEmit` for TypeScript type checking
- General checks: trailing whitespace, EOF fixer, merge conflict markers, YAML/JSON syntax

Pre-commit hooks are optional -- CI is the authoritative gate. But they catch issues before you push.

### Web linting and formatting

The web frontend uses [Biome](https://biomejs.dev/) for linting and formatting:

```bash
cd web
npm run lint          # check for issues
npm run lint:fix      # auto-fix issues
npm run format        # format all files
npm run format:check  # check formatting without writing
```

## Getting Help

Open an issue or start a discussion on GitHub. Read the [architecture overview](../architecture/overview.md) and [ADRs](../decisions/) for context on design decisions.
