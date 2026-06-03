# V1 Launch Readiness

OwnPulse V1 is ready when the open application is stable enough to review and self-host, and the managed service is reliable enough to charge for hosting and storage.

## Application Scope

V1 includes:

- Account registration, login, invite flow, password reset, linked auth methods, and account deletion.
- Manual entry for check-ins, interventions, health records, observations, and lab results.
- iOS HealthKit sync, write-back confirmation, offline retry, and sync status.
- Dashboard, Explore, Analyze, Protocols, Sources, Settings, Admin, exports, and user-facing docs.
- JSON and CSV export paths.

V1 does not require:

- Dexcom, lab PDF parsing, FHIR export, or cooperative aggregate sharing.
- Full feature parity between iOS and web admin/power-user screens.
- Billing integration before the hosted service has proven operational readiness.

## Trust Requirements

- API docs, Pact contracts, web clients, and iOS endpoints match.
- Export and delete behavior is documented and tested.
- Telemetry is opt-in or explicitly documented; hidden analytics are not allowed.
- Hosted users and self-hosters get the same data portability semantics.
- Public copy distinguishes current features from roadmap features.

## Hosted-Service Requirements

- Staging and production deploys are repeatable.
- Database backups run on schedule and have a documented restore drill.
- Rollback is tested for API and web deploys.
- Monitoring covers API health, database health, storage pressure, failed jobs, and certificate expiry.
- Operational access is restricted to production needs and documented in policy.

## Release Gates

- Backend: formatting, clippy, SQLx prepare check, unit/integration/contract tests.
- Web app: type check, lint, unit tests, Playwright, production build, route bundle check.
- iOS: simulator tests, Maestro flows, release archive, and one physical-device HealthKit smoke test.
- Public site: production build, waitlist smoke test, no unresolved asset warnings.
- Infra: OpenTofu plan review, Helm render/lint, staging deploy, backup/restore validation.
