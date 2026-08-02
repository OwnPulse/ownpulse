# OwnPulse Open Data Schema

**Version:** 0.2.0
**License:** CC0 1.0 (Public Domain)
**Canonical source:** [`schema/open-schema.json`](open-schema.json)

This document describes the OwnPulse open data export format. Any application can read a valid OwnPulse export without license obligation.

## Top-Level Keys

| Key | Type | Description |
|-----|------|-------------|
| `schema_version` | `string` | Semantic version of the schema format (e.g. `"0.2.0"`). |
| `schema_url` | `string` | URL to the canonical schema definition in the repository. |
| `description` | `string` | Human-readable description of this export. |
| `exported_at` | `string \| null` | ISO 8601 timestamp of when the export was generated. `null` in the skeleton. |
| `health_records` | `array` | All wearable and device measurements. Each record has a `record_type`, `value`, `unit`, `source`, and `recorded_at` timestamp. Covers heart rate, HRV, weight, blood glucose, sleep, steps, and other HealthKit-mapped metrics. |
| `interventions` | `array` | Substance, medication, and supplement logs. Each entry has a `name` (freeform text, no validation), `dosage`, `unit`, `route`, `taken_at` timestamp, and an `updated_at` timestamp (added in `0032_protocol_dose_tracking.sql`) reflecting the last edit via `PATCH /interventions/:id`. |
| `daily_checkins` | `array` | Five 1-10 subjective scores per day: energy, mood, focus, stress, sleep quality. Each entry has a `date` and the five scores. |
| `lab_results` | `array` | Blood panel and laboratory data. Each result has a `test_name`, `value`, `unit`, `reference_range`, and `collected_at` timestamp. Externally-sourced rows also carry `source`, `source_id`, and (for FHIR/MyChart imports) the standard `loinc_code`. |
| `observations` | `array` | User-defined flexible data. Each observation has a `type` (`event_instant`, `event_duration`, `scale`, `symptom`, `note`, `context_tag`, `environmental`, `sleep`), a `name`, and a JSONB `value` whose shape depends on the type. Sleep data (duration, sleep stages, score) has no separate table — it is stored here as `type = 'sleep'` and is covered by this array, not a dedicated `sleep` key. |
| `protocols` | `array` | Reusable intervention protocol recipes (name, description, duration, status). Templates (`user_id = null`) are excluded from a user's export. Added in `0.2.0`. |
| `protocol_lines` | `array` | Per-substance schedule lines belonging to a `protocols` entry (via `protocol_id`): substance, dose, unit, route, time of day, and schedule pattern. Added in `0.2.0`. |
| `protocol_runs` | `array` | Executions of a protocol: start date, status, and notification preferences for that run. Added in `0.2.0`. |
| `protocol_doses` | `array` | Logged or skipped doses for a `protocol_lines` entry, optionally scoped to a `protocol_runs` entry via `run_id` (`null` for legacy pre-run dose logs). Includes `skip_reason` when skipped. Logged doses reference the created record via `intervention_id`, which points into `interventions`. Added in `0.2.0`. |
| `genetic_records` | `array` | SNP variants from 23andMe/AncestryDNA/VCF uploads. Present only if the user has uploaded genetic data (omitted, not an empty array, otherwise). This is the user exporting their own data for portability, unrelated to the `sharing_consents`-gated *cooperative* genetics dataset. |

## Notes

- All timestamps are ISO 8601 with timezone (TIMESTAMPTZ in the database).
- All IDs are UUIDs.
- The schema is additive: new keys may be added in future versions but existing keys will not be removed or renamed.
- This file's keys are the exhaustive set of top-level keys the export can produce; a test (`export::test_export_json_keys_match_open_schema` in `backend/api/tests/integration/export.rs`) asserts `GET /export/json`'s keys never drift from this file. Many other tables in this repository (`users`, `calendar_days`, `sharing_consents`, `user_auth_methods`, `explore_charts`, `observer_polls` and related, `export_jobs`, etc.) are **not yet** part of the export — see [data-model.md](../docs/architecture/data-model.md#export-coverage) for the full picture.
- This schema matches the structure in `db/migrations/0001_init.sql` plus subsequent additive migrations. When the database schema changes, this file is updated to match.
