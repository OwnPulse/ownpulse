# ADR-0008: Bidirectional HealthKit Sync with Cycle Prevention

**Date:** 2026-03-17
**Status:** Accepted
**Deciders:** OwnPulse founding team

---

## Context

OwnPulse reads health data from Apple HealthKit and wants to write data back to HealthKit as well. This creates a potential cycle: data written to HealthKit by OwnPulse gets read back by OwnPulse on the next sync, creating duplicate records and potentially an infinite loop.

Additionally, third-party apps (Garmin, Oura, Withings) already sync their data to HealthKit. When OwnPulse also integrates with those APIs directly, the same data may arrive via two paths: directly from the third-party API and indirectly via HealthKit. This creates a deduplication problem.

The sync model must:
- Write user-entered data back to HealthKit so it appears in the Health app and is accessible to other health apps
- Never reimport data that OwnPulse itself wrote
- Handle overlapping data from multiple sources (Garmin → HealthKit → OwnPulse vs. Garmin API → OwnPulse directly)
- Let users configure which sources they trust for each metric type
- Be explainable to users — "why do I have two HRV entries for this morning?"

---

## Decision

Implement bidirectional HealthKit sync with three distinct mechanisms:

1. **Cycle prevention via bundle ID filtering:** All data written to HealthKit by OwnPulse uses the app's bundle ID (`com.ownpulse.app`) as the HKSource. On read, records whose source bundle ID matches OwnPulse's are filtered out unconditionally. This rule is enforced in code and cannot be overridden by configuration.

2. **Write-back queue:** The backend maintains a `healthkit_write_queue` table. When a user enters data manually (or when data arrives from a non-HealthKit source that has a HealthKit mapping), the backend inserts a queue entry. The iOS app polls this endpoint on foreground/background refresh, writes to HealthKit, and confirms completion. Records with `source = 'healthkit'` are never added to the write-back queue — enforced unconditionally in the service layer.

3. **Source preferences and deduplication wizard:** When a third-party integration is connected that may already sync to HealthKit, a one-time overlap scan detects duplicate records (same metric type, within 60 seconds and 2% value tolerance, different source). The user chooses the preferred source per metric type. Preferences are stored in `source_preferences` and applied at query time.

---

## Alternatives Considered

### Write unique identifiers into HealthKit metadata

Write a UUID into each HealthKit record's metadata when OwnPulse creates it. On read, filter records that have this metadata key.

Considered but rejected as primary mechanism because:
- HealthKit metadata is not guaranteed to be preserved by all HealthKit operations.
- The bundle ID filter is more robust and is Apple's recommended pattern.
- Both mechanisms can coexist (belt and suspenders) but the bundle ID filter is sufficient.

### Don't write back to HealthKit at all

Keep OwnPulse as a read-only HealthKit consumer. Simpler — no cycle risk.

Rejected because:
- Users who manually enter weight in OwnPulse want it to appear in the Health app.
- The platform's philosophy is to be a first-class citizen in the Apple Health ecosystem, not a silo.
- Write-back to HealthKit enables other health apps to benefit from data entered in OwnPulse.

### Timestamp-based deduplication (no source preference system)

Drop the second record when two records for the same metric type are within a time window.

Rejected because:
- "Last write wins" or "first write wins" are both wrong in some cases — the user should decide which source to trust for HRV (Oura vs Apple Watch), not the system.
- Silent deduplication is confusing — users see one record where they expected to see data from both sources.
- The source preferences system makes the deduplication explicit and user-controlled.

### Real-time sync (HealthKit observer queries)

Use `HKObserverQuery` to receive real-time notifications when HealthKit data changes, and sync immediately.

Considered but deferred. HealthKit observer queries require the app to be backgrounded and are subject to iOS background execution limits. For Phase 1, on-launch and background refresh sync is sufficient. Real-time sync can be added later without changing the data model.

---

## Consequences

**Positive:**
- The cycle prevention rule is unconditional and enforced in code — there is no configuration path that can accidentally enable a sync cycle.
- Users retain agency over which source is authoritative for overlapping metrics.
- OwnPulse becomes a genuine Apple Health ecosystem participant — data entered here flows to other health apps.
- The write-back queue is auditable (confirmed_at timestamps, error logging) — users can see what was written and when.

**Negative / tradeoffs:**
- The deduplication wizard adds onboarding friction when connecting a new integration that overlaps with HealthKit.
- The write-back queue adds latency — manual entries don't appear in HealthKit immediately, only after the next iOS foreground or background refresh.
- Two sources of truth for the same metric type during the transition period (before source preferences are set) can be confusing.

**Risks:**
- A future iOS update changes HealthKit's bundle ID filtering behavior. Monitor HealthKit release notes. The unconditional rule in code means we will never accidentally cycle, but we might over-filter if bundle ID behavior changes.
- Users connect many integrations that all sync to HealthKit. The deduplication wizard becomes tedious for each one. Mitigate by making source preferences persistent and providing smart defaults ("Oura beats HealthKit for HRV" as a suggested default based on known data quality).

---

## References

- HealthKit HKSource documentation: https://developer.apple.com/documentation/healthkit/hksource
- HealthKit best practices (Apple): https://developer.apple.com/documentation/healthkit/protecting_user_privacy
- Garmin Health API: https://developer.garmin.com/health-api/overview/
- Oura API v2: https://cloud.ouraring.com/v2/docs
