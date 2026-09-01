# iOS App — AGENTS.md

## Ownership

**Owns:** `ios/`

**Does NOT own:** `backend/`, `web/`, `helm/`

## Build

```
xcodebuild build -scheme OwnPulse
```

## Test

```
xcodebuild test -scheme OwnPulse -destination 'platform=iOS Simulator,name=iPhone 16'
maestro test ios/maestro/flows/
```

Before pushing, run `scripts/ci-ios.sh` from the repo root — it mirrors
`.github/workflows/ios.yml` (Debug test build + Release whole-module compile)
so `sending`/region-isolation errors surface locally instead of at TestFlight
time. Maestro flows are not run in CI; run them manually when you touch a
flow they cover.

## Interface to Other Services

- Pact consumer contract at `pact/contracts/ios-backend.json`

## Common Patterns

- Swift 6, SwiftUI, targeting iOS 18 minimum
- GRDB for the offline queue / local persistence
- Swift Testing framework for unit and integration tests
- HealthKit access abstracted behind a protocol (never called directly)
- Network layer abstracted behind a protocol (enables test doubles)
- JWT stored in the Keychain only
- Maestro for end-to-end UI testing

## What NOT to Do

- **No third-party dependencies except GRDB** — everything else is first-party Apple frameworks.
- **No third-party charting libraries** — use Swift Charts only (Phase 3b).
- **No storing JWT in `UserDefaults`** — use the Keychain.
- **No text matching in UI tests** — use accessibility identifiers for element lookup.
