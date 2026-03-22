# Data Model Reference

**Source of truth:** `db/migrations/0001_init.sql`

When the database schema changes, this document and `schema/open-schema.json` must be updated in the same PR.

## Tables

### `users`

User accounts.

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | `gen_random_uuid()` |
| `email` | TEXT | Unique |
| `display_name` | TEXT | |
| `data_region` | TEXT | `us` or `eu` |
| `created_at` | TIMESTAMPTZ | |
| `updated_at` | TIMESTAMPTZ | |

### `user_auth_methods`

Maps users to their linked authentication providers. A user can have multiple methods (e.g. local password + Apple Sign-In). Populated from existing `users.auth_provider` data by migration 0008.

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | `gen_random_uuid()` |
| `user_id` | UUID FK | References `users`, `ON DELETE CASCADE` |
| `provider` | TEXT | `local`, `google`, or `apple` |
| `provider_subject` | TEXT | Provider-specific user ID (e.g. Apple `sub` claim); `user_id::TEXT` for local |
| `email` | TEXT nullable | Email associated with this auth method |
| `created_at` | TIMESTAMPTZ | |

**Unique constraints:** `(provider, provider_subject)`, `(provider, email)`.

### `health_records`

All wearable and device measurements (heart rate, HRV, weight, blood glucose, sleep, steps, etc.).

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `record_type` | TEXT | e.g. `heart_rate`, `hrv`, `weight`, `blood_glucose`, `sleep`, `steps` |
| `value` | DOUBLE | Numeric measurement |
| `unit` | TEXT | e.g. `bpm`, `ms`, `kg`, `mg/dL`, `hours`, `count` |
| `source` | TEXT | e.g. `healthkit`, `garmin`, `oura`, `manual` |
| `source_id` | TEXT | External ID for deduplication |
| `duplicate_of` | UUID FK nullable | References `health_records` if this is a detected duplicate |
| `recorded_at` | TIMESTAMPTZ | When the measurement was taken |
| `created_at` | TIMESTAMPTZ | |

#### Deduplication

Before inserting any health record, the API checks for existing records within a **60-second window** and **2% value tolerance** from a different source. When a potential duplicate is detected:

- The new record is still inserted, but with its `duplicate_of` column set to reference the existing record's ID. Records are never silently dropped.
- The `source_preferences` table determines which source is preferred for each metric type. The preferred source's record is treated as canonical.
- A structured warning is logged containing both record IDs and their respective sources, enabling audit and debugging.

### `interventions`

Substance, medication, and supplement logs. Names are freeform text with no validation (non-judgmental by design).

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `name` | TEXT | Freeform, never validated |
| `dosage` | DOUBLE nullable | |
| `unit` | TEXT nullable | e.g. `mg`, `ml`, `iu` |
| `route` | TEXT nullable | e.g. `oral`, `sublingual`, `injection` |
| `taken_at` | TIMESTAMPTZ | |
| `created_at` | TIMESTAMPTZ | |

### `daily_checkins`

Five 1-10 subjective scores per day.

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `date` | DATE | One per user per day |
| `energy` | INT | 1-10 |
| `mood` | INT | 1-10 |
| `focus` | INT | 1-10 |
| `stress` | INT | 1-10 |
| `sleep_quality` | INT | 1-10 |
| `created_at` | TIMESTAMPTZ | |

### `lab_results`

Blood panel and laboratory data with reference ranges.

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `test_name` | TEXT | e.g. `TSH`, `Vitamin D`, `HbA1c` |
| `value` | DOUBLE | |
| `unit` | TEXT | |
| `reference_low` | DOUBLE nullable | |
| `reference_high` | DOUBLE nullable | |
| `collected_at` | TIMESTAMPTZ | |
| `created_at` | TIMESTAMPTZ | |

### `observations`

Flexible extensibility layer for user-defined data. See [ADR-0002](../decisions/0002-hybrid-schema.md).

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `type` | TEXT | `event_instant`, `event_duration`, `scale`, `symptom`, `note`, `context_tag`, `environmental` |
| `name` | TEXT | User-defined freeform name |
| `value` | JSONB | Shape depends on `type` (validated in API layer) |
| `started_at` | TIMESTAMPTZ | |
| `ended_at` | TIMESTAMPTZ nullable | For `event_duration` only |
| `created_at` | TIMESTAMPTZ | |

**JSONB `value` shapes by type:**

- `event_instant`: `{}` or `{"notes": "..."}`
- `event_duration`: `{}` or `{"notes": "..."}`
- `scale`: `{"numeric": 6, "max": 10}`
- `symptom`: `{"severity": 4}`
- `note`: `{"text": "..."}`
- `context_tag`: `{}`
- `environmental`: `{"numeric": 22.5, "unit": "celsius"}`

### `calendar_days`

Meeting and schedule aggregates per day.

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `date` | DATE | |
| `meeting_count` | INT | |
| `meeting_hours` | DOUBLE | |
| `created_at` | TIMESTAMPTZ | |

### `genetic_records`

SNP variant data, stored verbatim. Requires separate sharing consent (`dataset = 'genetics'`).

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `rsid` | TEXT | SNP identifier |
| `genotype` | TEXT | Stored verbatim, no interpretation |
| `source` | TEXT | e.g. `23andme`, `ancestry` |
| `created_at` | TIMESTAMPTZ | |

### `source_preferences`

Per-metric preferred source of truth. Used to resolve duplicates when multiple sources provide the same metric.

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `metric_type` | TEXT | e.g. `heart_rate`, `hrv`, `weight` |
| `preferred_source` | TEXT | e.g. `oura`, `garmin`, `healthkit` |
| `created_at` | TIMESTAMPTZ | |

### `healthkit_write_queue`

Pending HealthKit write-backs. Records with `source = 'healthkit'` are never added to this queue.

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `record_id` | UUID FK | References the source record |
| `record_table` | TEXT | Which table the record came from |
| `healthkit_type` | TEXT | HealthKit type identifier |
| `status` | TEXT | `pending`, `written`, `failed` |
| `confirmed_at` | TIMESTAMPTZ nullable | |
| `error` | TEXT nullable | |
| `created_at` | TIMESTAMPTZ | |

### `integration_tokens`

OAuth tokens for all third-party integrations. Encrypted with AES-256-GCM.

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `provider` | TEXT | e.g. `garmin`, `oura`, `dexcom`, `google` |
| `access_token_encrypted` | BYTEA | AES-256-GCM encrypted |
| `refresh_token_encrypted` | BYTEA | AES-256-GCM encrypted |
| `expires_at` | TIMESTAMPTZ nullable | |
| `created_at` | TIMESTAMPTZ | |
| `updated_at` | TIMESTAMPTZ | |

### `refresh_tokens`

JWT refresh tokens for app authentication.

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `token_hash` | TEXT | bcrypt hash of the refresh token |
| `expires_at` | TIMESTAMPTZ | |
| `created_at` | TIMESTAMPTZ | |

### `sharing_consents`

Cooperative data sharing consent. This is the trust boundary for all aggregate queries.

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `dataset` | TEXT | e.g. `health`, `genetics` (separate, stricter) |
| `consented_at` | TIMESTAMPTZ | |
| `revoked_at` | TIMESTAMPTZ nullable | Revocation is immediate |
| `privacy_policy_version` | TEXT | Version user consented under |
| `created_at` | TIMESTAMPTZ | |

### `export_jobs`

Export audit log.

| Column | Type | Notes |
|--------|------|-------|
| `id` | UUID PK | |
| `user_id` | UUID FK | References `users` |
| `format` | TEXT | `json`, `csv`, `fhir` |
| `status` | TEXT | `pending`, `complete`, `failed` |
| `started_at` | TIMESTAMPTZ | |
| `completed_at` | TIMESTAMPTZ nullable | |
| `created_at` | TIMESTAMPTZ | |

## Relationships

- All tables reference `users.id` via `user_id`.
- `health_records.duplicate_of` self-references `health_records.id`.
- `healthkit_write_queue.record_id` references the source record (polymorphic via `record_table`).
- `sharing_consents` is checked before any cooperative aggregate query.
