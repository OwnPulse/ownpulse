# OwnPulse Open Data Schema

**Version:** 0.1.0
**License:** CC0 1.0 (Public Domain)
**Canonical source:** [`schema/open-schema.json`](open-schema.json)

This document describes the OwnPulse open data export format. Any application can read a valid OwnPulse export without license obligation.

## Top-Level Keys

| Key | Type | Description |
|-----|------|-------------|
| `schema_version` | `string` | Semantic version of the schema format (e.g. `"0.1.0"`). |
| `schema_url` | `string` | URL to the canonical schema definition in the repository. |
| `description` | `string` | Human-readable description of this export. |
| `exported_at` | `string \| null` | ISO 8601 timestamp of when the export was generated. `null` in the skeleton. |
| `user` | `object` | User profile metadata (display name, data region, account creation date). No PII beyond what the user explicitly entered. |
| `health_records` | `array` | All wearable and device measurements. Each record has a `record_type`, `value`, `unit`, `source`, and `recorded_at` timestamp. Covers heart rate, HRV, weight, blood glucose, sleep, steps, and other HealthKit-mapped metrics. |
| `interventions` | `array` | Substance, medication, and supplement logs. Each entry has a `name` (freeform text, no validation), `dosage`, `unit`, `route`, and `taken_at` timestamp. |
| `daily_checkins` | `array` | Five 1-10 subjective scores per day: energy, mood, focus, stress, sleep quality. Each entry has a `date` and the five scores. |
| `lab_results` | `array` | Blood panel and laboratory data. Each result has a `test_name`, `value`, `unit`, `reference_range`, and `collected_at` timestamp. |
| `observations` | `array` | User-defined flexible data. Each observation has a `type` (`event_instant`, `event_duration`, `scale`, `symptom`, `note`, `context_tag`, `environmental`), a `name`, and a JSONB `value` whose shape depends on the type. |
| `calendar_days` | `array` | Meeting and schedule aggregates per day. Each entry has a `date`, `meeting_count`, and `meeting_hours`. |
| `sharing_consents` | `array` | Records of cooperative data sharing consent. Each entry has a `dataset`, `consented_at`, and `revoked_at` (null if active). |

## Notes

- All timestamps are ISO 8601 with timezone (TIMESTAMPTZ in the database).
- All IDs are UUIDs.
- The schema is additive: new keys may be added in future versions but existing keys will not be removed or renamed.
- `genetic_records` are excluded from the default export and require separate consent.
- This schema matches the structure in `db/migrations/0001_init.sql`. When the database schema changes, this file is updated to match.
