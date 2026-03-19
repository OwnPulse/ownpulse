# ADR-0002: Hybrid Schema Model (Structured Tables + Flexible Observations)

**Date:** 2026-03-17
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

OwnPulse needs to store a wide variety of health-related data. Some of this data is well-understood with fixed semantics — weight in kilograms, HRV in milliseconds, blood glucose in mg/dL, sleep duration in hours. These types have HealthKit mappings, FHIR representations, reference ranges, and unit conversions that are meaningful to encode structurally.

Other data is inherently open-ended. A user might want to track "sauna sessions," "fasted state," "joint stiffness on a 1-10 scale," "migraine severity," "travel days," or "cold plunge temperature" — none of which can be enumerated in advance. The platform's cooperative value proposition depends partly on this flexibility: users who find something useful to track should be able to track it without waiting for a schema update.

Additionally, one of the platform's differentiating commitments is being non-judgmental about interventions. Substance names in the intervention log must not be validated against a predefined list. This reinforces the need for a flexible data layer.

The schema design needs to serve:
- HealthKit bidirectional sync (requires type-safe mappings)
- FHIR R4 export (requires structured fields with known semantics)
- Cooperative aggregate queries (requires consistent naming across users)
- User-defined metrics (requires flexibility without schema migrations for each new type)
- AI agent development (requires the schema to be navigable and consistent)

---

## Decision

Use a **hybrid schema model**:

**Structured tables** for data with well-defined semantics, HealthKit mappings, or FHIR representations:
- `health_records` — all wearable and device measurements
- `interventions` — substance/medication/supplement logs
- `daily_checkins` — five fixed 1-10 subjective scores
- `lab_results` — blood panel data with reference ranges
- `calendar_days` — meeting aggregates
- `genetic_records` — SNP variant data

**One flexible table** (`observations`) for all user-defined data types, using a `type` discriminator column and a JSONB `value` field:
- `event_instant`, `event_duration`, `scale`, `symptom`, `note`, `context_tag`, `environmental`

The boundary rule: **if a data type has a HealthKit identifier or a FHIR resource mapping, it belongs in a structured table. If a user invented it, it belongs in `observations`.**

Observation names are freeform text. The API provides autocomplete suggestions ranked by frequency across consenting cooperative members (anonymized counts only), creating de facto standard names organically.

---

## Alternatives Considered

### Option A: Everything in one unified table (EAV or JSONB)

Put all data in a single `observations` table with a `type` column and JSONB `value`. No separate structured tables.

Pros: maximum flexibility, single index, one export path, trivially extensible.

Rejected because:
- SQLx compile-time query checking doesn't work against JSONB fields — we lose the primary safety benefit of using SQLx in the first place.
- HealthKit write-back requires knowing the exact field names and types for each metric (e.g. weight as `Double` in kg). This is much harder to reason about when the value is opaque JSONB.
- FHIR export requires structured fields. Generating FHIR from JSONB requires custom parsing logic for every type, defeating the purpose.
- Reference ranges for lab results (out_of_range computed column) require typed numeric fields.
- Harder for AI agents to reason about — "what fields does a weight record have?" is unanswerable from the schema alone.

### Option B: Separate table per data type

Each primitive gets its own table: `events`, `scales`, `symptoms`, `notes`, `context_tags`, `environmental_readings`.

Pros: clean separation, full type safety for every field.

Rejected because:
- Adding a new user-defined type requires a migration, a new model struct, a new route handler, a new iOS screen, and a new web component. This is incompatible with the goal of letting users define arbitrary metrics.
- Timeline query becomes a UNION across 10+ tables, complex to maintain and extend.
- The schema grows unboundedly as users request new types.

### Option C: Chosen hybrid

Structured where semantics are known, flexible where they aren't. The `observations` table handles the open-ended case; structured tables handle the cases where type safety, HealthKit sync, and FHIR export create real requirements.

---

## Consequences

**Positive:**
- SQLx compile-time checking applies to all structured tables — the high-value, frequently-queried data is fully type-safe.
- HealthKit write-back is straightforward for structured types — the mapping is explicit in code.
- FHIR export is clean — structured fields map directly to FHIR resource fields.
- New user-defined observation types require no schema migration — add a `type` value to the API validator and a UI component.
- The `observations` table's name autocomplete enables cooperative value (aggregate naming) without mandating taxonomy.
- Timeline query merges a bounded number of structured tables with one flexible table — manageable.

**Negative / tradeoffs:**
- Two mental models to hold simultaneously — structured and flexible. Contributors need to know which to use for new data types.
- The boundary rule ("HealthKit mapping or FHIR resource = structured") requires judgment for edge cases.
- JSONB validation for observations happens in the API layer, not the DB — less safety than a fully typed schema.
- Correlation queries that cross structured and observation data (e.g. correlate weight with a custom scale) require JOINs between the two layers.

**Risks:**
- Observation type proliferation: users invent hundreds of slightly different names for the same concept, making cooperative aggregates meaningless. Mitigate with the autocomplete/suggestion system encouraging convergence on shared names.
- Misclassification of a new data type (putting something in observations that should be structured, or vice versa). Mitigate with the clear boundary rule in CLAUDE.md and ADR review for new data types.

---

## References

- Martin Fowler on EAV anti-pattern: https://martinfowler.com/bliki/Evonsion.html
- PostgreSQL JSONB performance: https://www.postgresql.org/docs/current/datatype-json.html
- FHIR R4 resource types: https://hl7.org/fhir/R4/resourcelist.html
- HealthKit data types: https://developer.apple.com/documentation/healthkit/data_types
