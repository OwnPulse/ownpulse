# Bidirectional HealthKit Sync

OwnPulse reads from and writes to Apple HealthKit. This document covers the sync design, cycle prevention, write-back queue, and deduplication. See [ADR-0008](../decisions/0008-healthkit-sync.md) for the full rationale.

## Cycle Prevention

When OwnPulse writes a record to HealthKit, it uses the app's bundle ID (`health.ownpulse.app`) as the `HKSource`. On every HealthKit sync read (anchored queries and observer queries), records whose source bundle ID matches OwnPulse's are filtered out unconditionally.

This rule is:
- Enforced in the iOS `HealthKitProvider` implementation
- Not configurable
- Not overridable by any parameter or setting

This prevents the cycle: OwnPulse writes to HealthKit, then reads the same record back, creating a duplicate.

One consequence: HealthKit is no longer an implicit backup for data OwnPulse itself wrote there — sync reads never see it again. The backend export is the recovery path for that data, not HealthKit.

**Implementation:** `HealthKitProvider.makeReadPredicate()` builds the exclusion predicate (`NOT predicateForObjects(from: HKSource.default())`) and is applied, via `HealthKitProvider.makeAnchoredQuery()`, to every `HKAnchoredObjectQuery` in `HealthKitProvider.querySamples`, and directly to the `HKObserverQuery` in `HealthKitProvider.observeSampleUpdates()`. `MedicationSyncProvider` does not apply it — OwnPulse only reads dose events (no write-back path exists), so there's no cycle to guard against for that type. The file is gated behind `#if swift(>=6.3)` (live on the pinned Xcode 26.6 / Swift 6.3 toolchain; only runs on iOS 26+ devices). `ClinicalRecordProvider` does not apply it either — OwnPulse requests read-only clinical record access (`toShare: []`) and third-party apps cannot write `HKClinicalRecord`s, so there is no write → read cycle for that type today.

## Write-Back Queue Flow

```
User enters data        Third-party sync
(manual or API)         (Garmin, Oura, etc.)
       │                        │
       ▼                        ▼
   Backend API ── inserts record ──> health_records
       │
       ├── source = 'healthkit'?  ──> NO queue entry (unconditional)
       │
       └── source != 'healthkit'  ──> ALWAYS enqueued (no HealthKit-mapping
               │                       check on the backend — see note below)
               ▼
       healthkit_write_queue (pending: confirmed_at IS NULL AND failed_at IS NULL)
               │
               ▼
       iOS app polls GET /api/v1/healthkit/write-queue
               │
               ▼
       iOS writes each item to HealthKit via HKHealthStore.save()
               │
       ┌───────┴────────┐
       ▼                ▼
   write succeeds    write fails
       │                │
       ▼                ▼
POST /api/v1/healthkit/confirm       POST /api/v1/healthkit/confirm
  {"ids": [queue_id, ...]}             {"ids": [], "failures": [{"id": queue_id, "error": "..."}]}
       │                │
       ▼                ▼
confirmed_at = now()   failed_at = now(), error = <client message, truncated to 500 chars>
```

There is a single endpoint, `POST /api/v1/healthkit/confirm` — there is no per-item `/write-queue/:id/confirm` route. A single request body carries both outcomes: successfully-written item ids in `ids`, and failed items (with the client's error text) in `failures`. Both fields are scoped to the caller's own queue rows — a request can only confirm or fail items belonging to the authenticated user.

`failures` is `#[serde(default)]` on the backend — clients built before this flow existed can omit it entirely and keep working; they simply never report failures, so permanently-unwritable items stay pending forever (see below).

The iOS app polls the write-back queue on:
- App foreground
- Background refresh
- Manual sync trigger

Marking an item failed removes it from `GET /write-queue`'s pending set (`WHERE confirmed_at IS NULL AND failed_at IS NULL`) just like confirming it does. This matters because `get_pending` caps results at 100 rows ordered by `scheduled_at ASC`: before failure reporting existed, a permanently-unwritable item (e.g. a HealthKit type whose authorization was revoked, or a `record_type` with no HealthKit mapping at all — see below) sat at the head of the queue forever and starved every item behind it. Reporting the failure retires the item instead.

**This retirement path is for deterministic failures only** — an item that can never succeed (no matching HealthKit type, permanently revoked authorization). The iOS client is expected to keep genuinely transient errors (e.g. a momentary `HKHealthStore.save()` failure, device locked) pending rather than reporting them as `failures`, so they're retried on the next poll instead of being retired. A UI surface for reviewing/retrying failed items is a tracked follow-up, not part of this change — today `failed_at`/`error` are queryable in the DB but have no client-facing affordance.

The iOS app's write-back queue screen (Settings) lets the user explicitly decline to write a pending item into Apple Health; a decline is also reported through `failures` (with a `"declined by user"` error), not `ids` — it permanently retires the item the same way a deterministic HealthKit write error does, since no sample was ever written either way.

## Deduplication on New Integration Connect

When a user connects a new integration (e.g., Garmin) that also syncs to HealthKit, the same data may arrive via two paths:

1. **Garmin API -> OwnPulse** (direct integration sync)
2. **Garmin -> HealthKit -> OwnPulse** (via HealthKit read)

### Overlap Detection

On new integration connect, the backend runs a one-time overlap scan:
- For each metric type the new source provides, query `health_records` for records within 60 seconds and 2% value tolerance from a different source.
- Present detected overlaps to the user.

### Source Preferences

The user selects the preferred source per metric type. Preferences are stored in `source_preferences` and applied at query time (not at ingest): rows are never mutated or deleted, so no other read path needs to change.

**Duplicates are collapsed to one canonical row in charts, stats, and dashboards; `source_preferences` chooses which. Export and record lists always keep both.** Specifically:

- Every dedup pair (`duplicate_of`) collapses to exactly one visible row for aggregate reads — `GET /explore/series`, `POST /explore/series` (batch), `POST /explore/batch-series`, `GET /dashboard/summary`, and the `/stats/*` correlation endpoints (they all read through `db::explore::query_series`, or `db::health_records::SOURCE_PREFERENCE_EXCLUSION` directly for the dashboard count). This collapse is unconditional, not merely "when a preference exists": with **no** preference set at all (every user's default state), the pair still collapses to one row — the original, first-arriving record — rather than double-counting both (two sources reporting the same sleep session would otherwise inflate a night's sleep to ~16h). If a preference exists and names either side of the pair, that named source's row is shown instead of the default original; a preference naming a source absent from the pair is a no-op (falls back to the default).
- `duplicate_of` is stamped on whichever row arrives **second** — it does not, by itself, indicate which row is non-canonical. `SOURCE_PREFERENCE_EXCLUSION` walks to the actual dedup partner in both directions so the result doesn't depend on arrival order.

**Always kept raw (both rows returned, unconditionally):** `GET /health-records`, `GET /export/json`, `GET /export/csv`, and the friend-shared data view (`GET /friends/:id/data`, which reads `db::health_records::list` directly) — provenance is never dropped from a user's own data, their export, or what they choose to share with a friend.

### Deduplication Rules

- Duplicate detection window: 60 seconds and 2% value tolerance.
- Duplicates are never silently dropped.
- When a duplicate is detected: log a structured warning with both record IDs and sources, insert the record with a `duplicate_of` reference.
- `source_preferences` determines which record is canonical in aggregate reads; absent a preference, the original (first-arriving) record is canonical by default (see "Source Preferences" above for the exact read paths this applies to and the ones that stay raw).
- `POST /source-preferences` validates `preferred_source` against the known set of health-record sources (`garmin`, `oura`, `manual`, `healthkit`) — an unrecognized value is rejected with `400`, not silently stored inert.

On the `POST /healthkit/sync` bulk path, this rule is enforced via **batched cross-source dedup**: one preflight `UNNEST`-driven `SELECT` looks up the closest existing non-`healthkit` record for every row in the batch, followed by one `INSERT ... SELECT FROM UNNEST(...)` that writes the whole batch with each row's `duplicate_of` set from the preflight result. Two DB round trips per batch, regardless of batch size — the rule holds for 100-record batches at the same fidelity as the previous per-record path.

### Batch Size Cap

`POST /healthkit/sync` accepts at most **500 records per call** (`MAX_HEALTHKIT_BATCH`). Larger batches are rejected with `400 Bad Request` before reaching the DB. iOS's own batch size (`SyncEngine.batchSize`) is also 500 — the two constants are meant to track each other and currently leave **zero headroom**: raising either one without the other either silently under-utilizes the client's chunking or starts rejecting the client's own batches with 400s. Change them together, and add a load test at the new ceiling before raising the backend cap.

### Response Shape

On success, returns `201 Created` with a JSON body:

```json
{ "received": 100, "inserted": 98, "duplicates": 2 }
```

- `received` — records the server accepted from the request body.
- `inserted` — rows actually written (post `ON CONFLICT DO NOTHING`). Same-source replays are not counted.
- `duplicates` — cross-source near-duplicates detected and marked via `duplicate_of`. These rows **are** included in `inserted` — they land with a `duplicate_of` reference to the existing non-`healthkit` row, they are not dropped.

iOS currently consumes the endpoint with `requestNoContent` and discards the body; the ack shape exists so the HTTP contract is honest and so a future sync-status UI can read the counts without a wire change.

### Write-Queue Item Wire Shape

`GET /api/v1/healthkit/write-queue` returns an array of pending items:

```json
{
  "id": "77777777-7777-7777-7777-777777777777",
  "user_id": "550e8400-e29b-41d4-a716-446655440001",
  "hk_type": "body_mass",
  "value": {
    "value": 82.5,
    "unit": "kg",
    "start_time": "2026-03-20T10:00:00Z",
    "end_time": "2026-03-20T10:00:00Z"
  },
  "scheduled_at": "2026-03-20T10:00:00Z",
  "confirmed_at": null,
  "failed_at": null,
  "error": null,
  "source_record_id": "550e8400-e29b-41d4-a716-446655440010",
  "source_table": "health_records"
}
```

`value` is JSONB and its shape is exactly `{value, unit, start_time, end_time}`, no more, no fewer keys. Every field except `start_time` is nullable — `HealthRecordRow.value`/`unit`/`end_time` are all `Option` in the DB model, and a record posted without them still enqueues, with those keys present and `null` (never omitted). This is the iOS decode contract: iOS must decode `value`/`unit`/`end_time` as optional and treat a null `value` as a fail-reportable item, not a decode crash.

This shape is pinned by two integration tests (`test_write_queue_shape_after_manual_record_insert`, `test_write_queue_shape_with_null_value_fields`), asserted key-by-key. The `ios-backend.json` Pact contract's write-queue interaction round-trips the same hardcoded JSONB via its provider-state seeder, but Pact v2 object matching is non-strict equality-of-example, not a schema check — it would not fail if a producer sub-key were renamed in `routes/health_records.rs` while the seeder's literal JSON stayed unchanged. The integration tests are the actual enforcement for this shape; the Pact contract is a consumer-facing example, not a second independent gate.

## Medication Dose Import (iOS 26+)

`MedicationSyncProvider` reads `HKMedicationDoseEvent`s (per-object read
authorization, opt-in from Settings) and `SyncEngine.syncMedicationDoses`
POSTs each taken dose to `/api/v1/interventions`. Dose and route are omitted
when HealthKit has no quantity or the medication's form doesn't imply a
route, and `fasted` is always omitted (HealthKit doesn't record it) — the
client never fabricates values.

**Duplicate prevention is two-layered.**

- Server: interventions carry `source`/`source_id`
  (`0037_interventions_source_dedup.sql`), and the medication sync sends
  `source: "healthkit"` with the dose-event UUID as `source_id`. A replayed
  POST returns 200 with the existing row instead of inserting. HealthKit
  sample UUIDs sync across a user's devices via iCloud, so this dedupes
  reinstalls and multi-device setups alike — with one boundary: rows synced
  before 0037 shipped have `source_id = NULL` and are never deduplicated
  retroactively (the UUID was never sent).
- Client: two pieces of state in the GRDB anchor store cut replay chatter —
  `medication_dose_event` (the HK anchored-query anchor) and
  `medication_dose_posted_ids` (dose-event UUIDs uploaded during a pass
  whose anchor hasn't been saved yet; persisted after every successful POST
  and reset when the anchor saves).

The import is copy/append-only, not a mirror: a dose edited in Apple Health
as delete-and-re-add gets a new UUID and imports as a new intervention; the
original is not updated or removed. The reverse also holds: deleting an
imported intervention in OwnPulse doesn't touch Apple Health, and a later
full re-read (anchor reset) re-imports the dose unless it was also removed
there.

## HealthKit Type Mappings

Each structured `health_records.record_type` maps to a HealthKit type identifier. The mapping is maintained in the iOS `HealthKitProvider` implementation — **the backend has no knowledge of it** and enqueues every non-`healthkit`-sourced record for write-back unconditionally (see the Write-Back Queue Flow diagram above). A record type iOS can't map to a HealthKit identifier is retired client-side via the `failures` mechanism (report it as a deterministic failure on first encounter) rather than being filtered server-side.

## References

- [ADR-0008: Bidirectional HealthKit Sync](../decisions/0008-healthkit-sync.md)
- [Apple HealthKit HKSource documentation](https://developer.apple.com/documentation/healthkit/hksource)
