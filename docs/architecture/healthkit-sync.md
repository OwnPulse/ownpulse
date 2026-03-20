# Bidirectional HealthKit Sync

OwnPulse reads from and writes to Apple HealthKit. This document covers the sync design, cycle prevention, write-back queue, and deduplication. See [ADR-0008](../decisions/0008-healthkit-sync.md) for the full rationale.

## Cycle Prevention

When OwnPulse writes a record to HealthKit, it uses the app's bundle ID (`com.ownpulse.app`) as the `HKSource`. On every HealthKit read, records whose source bundle ID matches OwnPulse's are filtered out unconditionally.

This rule is:
- Enforced in the iOS `HealthKitProvider` implementation
- Not configurable
- Not overridable by any parameter or setting

This prevents the cycle: OwnPulse writes to HealthKit, then reads the same record back, creating a duplicate.

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
       healthkit_write_queue (status: pending)
               │
               ▼
       iOS app polls GET /api/v1/healthkit/write-queue
               │
               ▼
       iOS writes to HealthKit via HKHealthStore.save()
               │
               ▼
       iOS calls POST /api/v1/healthkit/write-queue/:id/confirm
               │
               ▼
       Queue entry updated (status: written, confirmed_at set)
```

The iOS app polls the write-back queue on:
- App foreground
- Background refresh
- Manual sync trigger

Failed writes are retried on the next poll. Errors are logged with the queue entry.

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

## HealthKit Type Mappings

Each structured `health_records.record_type` maps to a HealthKit type identifier. The mapping is maintained in the iOS `HealthKitProvider` implementation. Only record types with a known HealthKit mapping are eligible for write-back.

## References

- [ADR-0008: Bidirectional HealthKit Sync](../decisions/0008-healthkit-sync.md)
- [Apple HealthKit HKSource documentation](https://developer.apple.com/documentation/healthkit/hksource)
