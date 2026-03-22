# System Architecture Overview

OwnPulse is three services behind one REST API, deployed on Kubernetes (k3s).

## Services

| Service | Stack | Deployment | URL pattern |
|---------|-------|------------|-------------|
| **Backend API** | Rust + Axum + SQLx + Postgres 17 | Rust binary container | `api.<domain>` |
| **Web frontend** | React + Vite + TypeScript + unovis | nginx container serving static build | `app.<domain>` |
| **iOS app** | Swift 6 + SwiftUI + HealthKit | App Store / TestFlight | native device |

All three consume the same REST API with JWT authentication. The web and iOS clients share the same auth flow and token format.

## Data Flow

```
HealthKit  <──>  iOS app  ──>  Backend API  <──  Web frontend
                                    │
                                    ▼
                              PostgreSQL 17
```

- The iOS app reads from and writes to HealthKit (bidirectional sync, see [healthkit-sync.md](healthkit-sync.md)).
- The backend is the single source of truth for all data.
- Third-party integrations (Garmin, Oura, Dexcom) sync via background jobs in the backend.

## Deployment

- **Production/staging:** k3s on Linux VPS (DigitalOcean US, Hetzner EU in Phase 2).
- **Local development:** k3d (k3s inside Docker) or individual services via Docker + cargo + npm.
- **Self-hosting:** k3s on any Linux VPS, same Helm charts as production.

One set of Helm charts (`helm/`) works across all environments. See [ADR-0006](../decisions/0006-k3d-kubernetes.md) for the rationale.

## CI/CD

- Linux jobs (backend, web): ARC ephemeral runner pods on the k3s cluster.
- iOS jobs: Self-hosted Mac mini M4 with Tart VMs. See [ADR-0007](../decisions/0007-macos-ci.md).
- Deploy: `helm upgrade` on merge to main when backend + web CI passes.

## Key ADRs

- [ADR-0001: Rust and Axum](../decisions/0001-rust-axum.md)
- [ADR-0002: Hybrid schema model](../decisions/0002-hybrid-schema.md)
- [ADR-0004: React + unovis](../decisions/0004-react-unovis.md)
- [ADR-0005: iOS as data pump](../decisions/0005-ios-data-pump.md)
- [ADR-0006: k3s/k3d deployment](../decisions/0006-k3d-kubernetes.md)
- [ADR-0009: Testing strategy](../decisions/0009-testing-strategy.md)
