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

**Implementation:** `HealthKitProvider.makeReadPredicate()` builds the exclusion predicate (`NOT predicateForObjects(from: HKSource.default())`) and is applied, via `HealthKitProvider.makeAnchoredQuery()`, to every `HKAnchoredObjectQuery` in `HealthKitProvider.querySamples`, and directly to the `HKObserverQuery` in `HealthKitProvider.observeSampleUpdates()`. `MedicationSyncProvider` does not apply it — OwnPulse only reads dose events (no write-back path exists), so there's no cycle to guard against for that type, and the file is gated behind `#if swift(>=6.3)` (inert on the pinned Swift 6.0 toolchain). `ClinicalRecordProvider` does not apply it either — OwnPulse requests read-only clinical record access (`toShare: []`) and third-party apps cannot write `HKClinicalRecord`s, so there is no write → read cycle for that type today.

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
       └── source != 'healthkit' AND has HealthKit mapping?
               │
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

Marking an item failed removes it from `GET /write-queue`'s pending set (`WHERE confirmed_at IS NULL AND failed_at IS NULL`) just like confirming it does. This matters because `get_pending` caps results at 100 rows ordered by `scheduled_at ASC`: before failure reporting existed, a permanently-unwritable item (e.g. a HealthKit type whose authorization was revoked) sat at the head of the queue forever and starved every item behind it. Reporting the failure retires the item instead.

## Deduplication on New Integration Connect

When a user connects a new integration (e.g., Garmin) that also syncs to HealthKit, the same data may arrive via two paths:

1. **Garmin API -> OwnPulse** (direct integration sync)
2. **Garmin -> HealthKit -> OwnPulse** (via HealthKit read)

### Overlap Detection

On new integration connect, the backend runs a one-time overlap scan:
- For each metric type the new source provides, query `health_records` for records within 60 seconds and 2% value tolerance from a different source.
- Present detected overlaps to the user.

### Source Preferences

The user selects the preferred source per metric type. Preferences are stored in `source_preferences` and applied at query time (not at ingest). Both records are kept; the non-preferred source is deprioritized in the default view.

### Deduplication Rules

- Duplicate detection window: 60 seconds and 2% value tolerance.
- Duplicates are never silently dropped.
- When a duplicate is detected: log a structured warning with both record IDs and sources, insert the record with a `duplicate_of` reference.
- `source_preferences` determines which record is shown by default.

On the `POST /healthkit/sync` bulk path, this rule is enforced via **batched cross-source dedup**: one preflight `UNNEST`-driven `SELECT` looks up the closest existing non-`healthkit` record for every row in the batch, followed by one `INSERT ... SELECT FROM UNNEST(...)` that writes the whole batch with each row's `duplicate_of` set from the preflight result. Two DB round trips per batch, regardless of batch size — the rule holds for 100-record batches at the same fidelity as the previous per-record path.

### Batch Size Cap

`POST /healthkit/sync` accepts at most **500 records per call** (`MAX_HEALTHKIT_BATCH`). Larger batches are rejected with `400 Bad Request` before reaching the DB. iOS chunks by 100 records, so the cap leaves ~5x headroom. Raising the limit requires a load test at the new ceiling.

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

`value` is JSONB and its shape — exactly `{value, unit, start_time, end_time}`, no more, no fewer keys — is the iOS decode contract. It is populated verbatim by the service-layer enqueue call at insertion time (see `db_healthkit::enqueue_write` callers) and is pinned by an integration test (`test_write_queue_shape_after_manual_record_insert`) and the `ios-backend.json` Pact contract, since a prior mismatch between this shape and the iOS decoder shipped undetected.

## HealthKit Type Mappings

Each structured `health_records.record_type` maps to a HealthKit type identifier. The mapping is maintained in the iOS `HealthKitProvider` implementation. Only record types with a known HealthKit mapping are eligible for write-back.

## References

- [ADR-0008: Bidirectional HealthKit Sync](../decisions/0008-healthkit-sync.md)
- [Apple HealthKit HKSource documentation](https://developer.apple.com/documentation/healthkit/hksource)
