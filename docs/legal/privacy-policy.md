# Privacy Policy

> **DRAFT -- This document requires review by qualified legal counsel before publication. It is not a legally binding document in its current form.**

**Last updated:** 2026-03-18

## Who We Are

OwnPulse is a data cooperative that provides an open source health data platform. This privacy policy covers the hosted cooperative instance at ownpulse.health. Self-hosted instances are governed by the operator's own privacy policy.

## What Data We Collect

### Data You Provide

- **Account information:** Email address, display name, chosen data region.
- **Health records:** Wearable measurements, manual entries (heart rate, HRV, weight, blood glucose, sleep, steps, etc.).
- **Interventions:** Substance, medication, and supplement logs.
- **Daily check-ins:** Subjective scores (energy, mood, focus, stress, sleep quality).
- **Lab results:** Blood panel data.
- **Observations:** User-defined flexible data (events, scales, symptoms, notes, context tags, environmental readings).
- **Genetic records:** SNP variant data (only if you upload it).

### Data From Connected Services

When you connect third-party integrations (Garmin, Oura, Dexcom, Apple HealthKit), we receive the data those services provide via their APIs. We store OAuth tokens encrypted with AES-256-GCM.

### Data We Do Not Collect

- Usage analytics
- Crash reports (unless you enable them)
- Behavioral data or telemetry
- IP-based geolocation
- Advertising identifiers

## How We Store Your Data

- Your data is stored on infrastructure in your chosen region (US or EU).
- EU user data is stored on EU infrastructure (Hetzner, Germany). US user data is stored on US infrastructure (DigitalOcean).
- No cross-region replication of personal data.
- OAuth tokens are encrypted at rest with AES-256-GCM.
- All connections use TLS.

## How We Share Your Data

### We Do Not Share Your Data By Default

Your data is never shared, sold, or used for advertising. Nothing leaves your account without your explicit consent.

### Cooperative Data Sharing (Opt-In)

If you choose to participate in the cooperative research marketplace:
- You grant consent per dataset (health records, genetic data, etc.).
- Your data is anonymized before aggregation (k-anonymity k>=50, differential privacy, no quasi-identifiers).
- Researchers receive aggregate query access only, never raw individual records.
- 80% of research revenue flows to consenting users; 20% covers operating costs.
- You can revoke consent at any time. Revocation is immediate.

## Your Rights

### Under GDPR (EU Users)

- **Access:** Export all your data at any time (JSON, CSV, FHIR R4).
- **Erasure:** Delete your account. All records are anonymized within 72 hours.
- **Portability:** Export in machine-readable open formats.
- **Restriction:** Suspend processing of specific datasets without deletion.
- **Objection:** Revoke cooperative sharing consent immediately.

### Under CCPA (California Users)

- **Right to know:** Full data export available.
- **Right to delete:** Account deletion available.
- **Right to opt-out of sale:** Cooperative sharing is opt-in only.
- **No discrimination:** Full functionality regardless of sharing choices.

### Under PIPEDA (Canadian Users)

- **Consent:** Meaningful consent for all data collection and sharing.
- **Access:** Full data access at any time.
- **Accuracy:** Correct your data at any time.

## Data Retention

- Data is retained as long as your account is active.
- On account deletion, personal data is anonymized within 72 hours.
- Audit logs (export history, consent changes) are retained for legal compliance.
- Anonymized aggregate data that was shared before consent revocation cannot be un-shared.

## Contact

For privacy inquiries, contact: privacy@ownpulse.health

## Changes to This Policy

We will notify users of material changes via email. The privacy policy version is recorded with each consent grant.
