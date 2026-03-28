# ADR-0011: Explore Charts, Observer Polls, and Real-Time Updates

**Status:** Accepted
**Date:** 2026-03-28
**Authors:** Tony + Claude

## Context

The current charting (WeightChart, SleepChart on Timeline page) is minimal — no date range control, no resolution toggle, no multi-metric overlay, no interactivity. The app needs a proper data exploration experience as its core feature. Additionally, users want external subjective assessments from trusted people (partners, coaches) to compare against self-reported check-in scores. Finally, charts should update in real-time when new data arrives.

## Decisions

### 1. Explore Page replaces Timeline

A new `/explore` page with:
- **Time range control**: presets (7d, 30d, 90d, 1y, all) + custom date picker
- **Resolution toggle**: daily, weekly, monthly aggregation via `date_trunc`
- **Metric picker**: categorized list of all available metrics
- **Multi-metric overlay**: up to 8 metrics on one chart, dual Y-axes for incompatible units
- **Intervention markers**: vertical lines showing substance + dose on the time axis
- **Tooltips**: value, unit, time, source on hover/tap
- **Saved charts**: persist chart configs in `explore_charts` table, referenceable by UUID

### 2. Source-Field Matrix as Rust Enum Allowlist

The `explore/series` endpoint maps user-supplied `(source, field)` pairs to SQL queries. To prevent SQL injection, this mapping is a Rust enum — every valid combination has a dedicated, compile-time-checked `sqlx::query_as!` call. No string interpolation into SQL ever occurs.

| Source | Fields | Table |
|--------|--------|-------|
| `health_records` | heart_rate, hrv, body_mass, blood_glucose, etc. | `health_records` (filtered by `record_type`) |
| `checkins` | energy, mood, focus, recovery, libido | `daily_checkins` (column per field) |
| `labs` | dynamic marker names | `lab_results` (parameterized `WHERE marker = $field`) |
| `calendar` | meeting_minutes, meeting_count | `calendar_days` |
| `sleep` | duration_minutes, deep_minutes, rem_minutes, score | `observations` (type='sleep', JSONB extraction) |
| `observer_polls` | `<poll_id>:<dimension>` | `observer_responses` (validated poll ownership + dimension) |

### 3. Observer Polls (Separate from Friend Shares)

Observer polls are a new feature allowing trusted people to submit daily 1-10 assessments about the user. Key design decisions:

- **Observers require OwnPulse accounts** (authenticated, not anonymous)
- **One-way data flow**: observers submit scores, never see the owner's health data
- **Separate from friend_shares**: different access semantics (one-way input vs bidirectional viewing)
- **Soft-delete polls**: `deleted_at` timestamp, no CASCADE on responses
- **Observer data rights**: observers can view, export, and delete their own responses
- **Account deletion**: anonymizes `observer_id` (SET NULL), responses preserved for owner
- **Invite tokens**: UUID v4 with 7-day expiry, uniform success response on accept (email enumeration prevention)
- **Three consent models documented**: cooperative sharing, friend sharing, observer polls

### 4. Real-Time Updates via SSE

`GET /api/v1/events?token=<JWT>` — Server-Sent Events stream. Backend publishes per-user events via `tokio::sync::broadcast` when data is written. Clients subscribe and invalidate query caches.

- JWT passed as query param (EventSource doesn't support custom headers)
- Keepalive every 30 seconds
- Cross-platform: web (EventSource), iOS (URLSession streaming), Android (OkHttp)

### 5. Batch Series Endpoint

`POST /api/v1/explore/series` accepts an array of metric specs, returns all series in one response. Reduces N HTTP round-trips for N overlaid metrics. Rate-limited at 30 req/min per user.

## Consequences

- Timeline page, WeightChart, and SleepChart are deleted
- Navigation changes: "Timeline" becomes "Explore", "Observer Polls" added
- Two new tables: `explore_charts`, plus three for observer polls
- SSE adds a long-lived connection per authenticated client
- All new endpoints require Pact contract updates when iOS/web consumers are written

## Phasing

- **A1**: Explore page + series endpoint + saved charts (web only)
- **B1**: Observer polls backend + frontend (parallel with A1)
- **A2**: Dashboard sparklines, pinned metric cards, intervention markers
- **B2**: Observer poll data in Explore metric picker
- **SSE**: Ships with A1

## Security Measures

- Source-field enum allowlist (no SQL injection)
- UUID v4 tokens with expiry (no brute-force)
- IDOR protection (user_id in all WHERE clauses)
- Email enumeration prevention (uniform responses)
- Rate limiting on explore (30/min) and poll accept/respond (10/min)
- Observer-to-owner data boundary enforced at DB query level
- Config JSONB validated against strict schema (no stored XSS)
