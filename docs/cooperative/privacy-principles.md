# Privacy Principles

OwnPulse is built on the principle that users own their health data. These principles are enforced in the architecture, not just the policy.

See [ADR-0010](../decisions/0010-data-cooperative.md) for the full rationale.

## Data Sovereignty

- **User-controlled storage.** Data is stored on cooperative infrastructure in the user's chosen region, or on the user's own infrastructure for self-hosted deployments.
- **No cross-region replication.** EU users' data stays on EU infrastructure (Hetzner, Germany). US users' data stays on US infrastructure (DigitalOcean). The `data_region` field is set at signup and enforced at the infrastructure level.
- **Nothing leaves without consent.** No data leaves the user's account without explicit opt-in. There is no background telemetry, no analytics, no usage tracking.

## Full Export

- Users can export 100% of their data at any time.
- Export formats: JSON (open schema), CSV, FHIR R4.
- Exports are streaming -- they work for any data volume without buffering.
- No artificial delays, no approval process, no data held hostage.
- The export schema is open (CC0 licensed) so any application can read it.

## Regulatory Compliance

OwnPulse implements the following regulatory frameworks:

### GDPR (EU)

- **Lawful basis:** Explicit consent (Article 9 for special category health data).
- **Data subject rights:** Access (full export), erasure (anonymize within 72h), portability (open formats), restriction (suspend processing), objection (revoke sharing).
- **Data residency:** EU data on EU infrastructure.
- **Sub-processor DPAs:** Required with all infrastructure providers before accepting EU users.

### CCPA (California)

- **Right to know:** Full export available.
- **Right to delete:** Account deletion anonymizes all records.
- **Right to opt-out of sale:** Cooperative data sharing is opt-in, not opt-out.
- **No discrimination:** No user is penalized or given reduced functionality based on their data sharing choices.

### PIPEDA (Canada)

- **Consent:** Meaningful consent for data collection and sharing.
- **Access:** Users can access all their data.
- **Accuracy:** Users can correct their data.
- **Retention:** Data is retained only as long as the account is active.

## No Analytics

OwnPulse does not collect:
- Usage analytics
- Crash reports (unless the user explicitly enables them)
- Telemetry
- Behavioral data
- IP-based geolocation

The platform has no tracking scripts, no analytics SDKs, and no third-party cookies.

## Encryption

- OAuth tokens for third-party integrations are encrypted at rest with AES-256-GCM.
- JWT refresh tokens are stored as bcrypt hashes.
- TLS in transit (enforced via cert-manager and Let's Encrypt).
- Database connections use TLS in production.
