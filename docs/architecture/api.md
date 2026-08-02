# API Reference

**Base URL:** `https://app.<domain>/api/v1`

All endpoints require JWT authentication unless marked as public. Tokens are issued via the auth endpoints and passed as `Authorization: Bearer <token>`.

Clients (web and iOS) send an optional `X-App-Version` request header carrying their build identifier (git SHA). The server records it as a span field on each request so stale clients are visible in logs; it is never required and never affects request handling.

## API versioning policy

The API is versioned in the URL path: `/api/v1`, `/api/v2`, and so on. A version
namespace groups a stable contract — clients pin to a version and are not broken
by additive changes within it.

- **Additive, backward-compatible changes** (new endpoints, new optional request
  fields, new response fields) are made in place within the current version. They
  do **not** require a new version.
- **Breaking changes** (removing or renaming a field, changing a field's type,
  changing status-code semantics, removing an endpoint) get a **parallel endpoint
  under the next version**. The `v1` equivalent stays live and gains a
  `Deprecation` response header pointing clients at the `v2` replacement.
- **Version support and removal.** The backend supports the current and the
  immediately-previous API version. An old version is removed only in a server
  release whose notes explicitly document the removal — never solely because a
  calendar period has elapsed. This matters for self-hosting: an operator may
  upgrade across several releases at once and never run the intermediate ones, so
  a wall-clock deprecation window would silently strip a version out from under a
  pinned client with no notice the operator ever saw. The `Deprecation` header
  remains an advisory hint for clients, not the removal trigger.
- `/api/v2` is currently a mounted but empty namespace (see
  `backend/api/src/routes/v2/mod.rs`). Requests under it return a clean `404`
  until the first `v2` endpoint ships. It exists so a breaking change can be
  introduced without restructuring the router.
- Consumer contracts in `pact/contracts/` continue to pin `v1`
  (`pact/contracts/ios-backend.json`, `pact/contracts/web-backend.json`). A
  contract is repointed to `v2` only when its consumer actually migrates to a
  `v2` endpoint — adding the `v2` namespace alone does not change any contract.
  Any `v2` endpoint called by web or iOS must have matching Pact coverage for the
  version that consumer calls, added before the consumer is switched to it.
  Backend-only or unconsumed `v2` endpoints need no contract.

## Implemented

This section is not exhaustive. Several route groups are live in
`routes/mod.rs` but not yet written up in detail here — including sleep,
saved medicines, insights, dashboard/summary, telemetry, config, audit
log, admin (invites/users/feature-flags), and the protocol-runs family.
Their absence from this document is a documentation gap, not evidence
they're unimplemented; check `routes/mod.rs` directly for the full current
route list. The **Planned — not implemented** section below is the
reliable list of what genuinely doesn't exist yet.

### Public

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/health` | Health check (public) | 1 |
| GET | `/health/telemetry` | Telemetry-ingest pipeline liveness (public ops probe) | 1 |
| POST | `/waitlist` | Waitlist signup (public) | 1 |

#### `GET /api/v1/health/telemetry`

Aggregate-only liveness signal for the telemetry-ingest pipeline. Lets an operator
confirm the backend is still receiving `app_events`. Unauthenticated — it is an
ops/monitoring surface (a sibling of `/readyz`), and it exposes **no** user
identity, device id, payload, or any health data. Counts and timestamps only.

**Response:** `200 OK`

```json
{
  "events_last_5m": 12,
  "last_event_at": "2026-06-03T10:15:00Z",
  "last_event_age_seconds": 42
}
```

When no events have ever been recorded, `last_event_at` and
`last_event_age_seconds` are `null` and `events_last_5m` is `0`.

**Errors:**

| Status | Reason |
|--------|--------|
| 503 | Telemetry stats query failed (database unavailable) — degraded, never a raw 500 |

**Metrics / alerting:** the handler emits the Prometheus gauge
`ownpulse_telemetry_last_event_age_seconds` (seconds since the most recent
`app_events` row). The intended `TelemetryStalled` alert (defined in
ownpulse-infra) fires when this gauge exceeds `1800` (30 minutes). The gauge is
left unset while the table is empty, so a fresh instance does not trip the alert
before any client has ever reported.

### Auth (Public)

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/auth/login` | Login with username/password, returns JWT + refresh token | 1 |
| POST | `/auth/register` | Register with invite code (see below) | 1 |
| POST | `/auth/refresh` | Refresh token rotation (cookie) | 1 |
| POST | `/auth/logout` | Invalidate refresh token | 1 |
| GET | `/auth/google/login` | Google OAuth redirect (accepts `?invite_code=`, `?mode=link`) | 1 |
| GET | `/auth/google/callback` | Google OAuth callback (login, register, or link) | 1 |
| POST | `/auth/apple/callback` | Apple Sign-In callback (verify id_token, issue tokens) | 1 |
| GET | `/auth/methods` | List auth methods linked to current user (requires JWT) | 1 |
| POST | `/auth/link` | Link a new auth provider to current user (requires JWT) | 1 |
| DELETE | `/auth/link/:provider` | Unlink an auth provider from current user (requires JWT) | 1 |

#### `POST /auth/register`

Register a new account. When the instance requires invites (`REQUIRE_INVITE=true`, the default), a valid invite code must be provided.

**First-user exception:** When the users table is empty (fresh instance), the first registration bypasses the invite requirement and the user is automatically promoted to admin. This is protected by a PostgreSQL advisory lock to prevent race conditions.

**Request body:**

```json
{
  "email": "string",
  "password": "string",
  "invite_code": "string"
}
```

**Response:** `TokenResponse` (same shape as `/auth/login`).

**Errors:**

| Status | Reason |
|--------|--------|
| 400 | Invalid or expired invite code, or validation failure |
| 409 | Email already registered |

#### Google OAuth with invite code

`GET /auth/google/login` accepts an optional `?invite_code=XYZ` query parameter. If the user does not yet have an account and invite codes are required, the invite code is validated during the OAuth callback. If no valid code is present, the callback returns a `400` JSON error (`"invite code required for new account registration"`).

#### Google OAuth account linking

`GET /auth/google/login` accepts an optional `?mode=link` query parameter. When present, the backend encodes a `:link` marker into the OAuth `state` parameter. On callback, the backend reads the marker and links the Google account to the currently authenticated user instead of performing a login or registration.

The user must have a valid session (JWT) when initiating the link flow. The backend reads the JWT from the `token` cookie (the same httpOnly cookie used for refresh tokens is not required -- the access token cookie is sufficient).

**Error redirects from `/auth/google/callback` during linking:**

| Condition | Redirect |
|-----------|----------|
| No valid session | `<WEB_ORIGIN>/login?error=auth_required` |
| Google email already linked to a different user | `<WEB_ORIGIN>/settings?error=already_linked` |
| Success | `<WEB_ORIGIN>/settings?linked=google` |

#### `POST /auth/apple/callback`

Verify an Apple Sign-In identity token and issue access + refresh tokens. Creates a new user if one does not exist for the Apple `sub` claim.

**Request body:**

```json
{
  "id_token": "string (Apple identity JWT)",
  "platform": "string (\"web\" or \"ios\")"
}
```

**Response (iOS / non-web):** `TokenResponseWithRefresh` — includes `refresh_token` in the JSON body for Keychain storage.

```json
{
  "access_token": "string",
  "refresh_token": "string",
  "token_type": "Bearer",
  "expires_in": 3600
}
```

**Response (web):** `TokenResponse` — refresh token is set as an httpOnly cookie only; not included in the body.

```json
{
  "access_token": "string",
  "token_type": "Bearer",
  "expires_in": 3600
}
```

**Errors:**

| Status | Reason |
|--------|--------|
| 400 | Unknown `platform` value (must be `"web"` or `"ios"`) |
| 401 | Identity token verification failed (invalid signature, expired, wrong audience, JWKS fetch error) |
| 409 | Email already registered with a different provider |
| 500 | `APPLE_CLIENT_ID` not configured |

#### `GET /auth/methods`

List all auth methods linked to the authenticated user's account. Requires JWT.

**Response:** `200 OK` — array of `AuthMethodRow`.

```json
[
  {
    "id": "uuid",
    "provider": "local",
    "email": "user@example.com",
    "created_at": "2026-03-21T00:00:00Z"
  },
  {
    "id": "uuid",
    "provider": "apple",
    "email": "user@privaterelay.appleid.com",
    "created_at": "2026-03-21T00:00:00Z"
  }
]
```

**Errors:**

| Status | Reason |
|--------|--------|
| 401 | Missing or invalid JWT |

#### `POST /auth/link`

Link a new auth provider to the authenticated user's account. Requires JWT.

**Request body:**

```json
{
  "provider": "string (\"apple\", \"local\", or \"google\")",
  "id_token": "string (required for apple)",
  "password": "string (required for local, min 8 characters)"
}
```

**Response:** `200 OK` — array of `AuthMethodRow` (updated list of all linked methods).

**Errors:**

| Status | Reason |
|--------|--------|
| 400 | Missing required field for provider, password too short, or unsupported provider. Google linking uses the OAuth redirect flow (`GET /auth/google/login?mode=link`) instead of this endpoint |
| 401 | Missing/invalid JWT, or Apple id_token verification failed |
| 409 | The Apple account is already linked to a different user |

#### `DELETE /auth/link/:provider`

Unlink an auth provider from the authenticated user's account. Users cannot unlink their last remaining login method. Requires JWT.

**Response:** `200 OK` — array of `AuthMethodRow` (updated list after removal).

**Errors:**

| Status | Reason |
|--------|--------|
| 400 | Cannot remove your only login method |
| 401 | Missing or invalid JWT |
| 404 | Provider not linked to this account |

### Admin -- Invite Management

All admin endpoints require JWT authentication with `role = admin`.

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/admin/invites` | Create an invite code | 1 |
| GET | `/admin/invites` | List all invite codes | 1 |
| DELETE | `/admin/invites/:id` | Revoke an invite code | 1 |

#### `POST /admin/invites`

**Request body:**

```json
{
  "label": "string (optional)",
  "max_uses": "number (optional)",
  "expires_in_hours": "number (optional)"
}
```

**Response:** `InviteCode`

```json
{
  "id": "uuid",
  "code": "string",
  "label": "string | null",
  "max_uses": "number | null",
  "use_count": 0,
  "expires_at": "timestamp | null",
  "revoked_at": null,
  "created_at": "timestamp"
}
```

#### `GET /admin/invites`

**Response:** `InviteCode[]`

#### `DELETE /admin/invites/:id`

Sets `revoked_at` on the invite code. Does not delete the record.

**Response:** `InviteCode` (with `revoked_at` set)

### Admin -- User Management

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| PATCH | `/admin/users/:id/status` | Enable or disable a user | 1 |
| DELETE | `/admin/users/:id` | Delete a user and all their data | 1 |

#### `PATCH /admin/users/:id/status`

**Request body:**

```json
{
  "status": "active | disabled"
}
```

**Response:** `UserResponse` (includes `status` field)

Disabled users are locked out immediately -- their next API request returns 403. Admins cannot disable themselves.

#### `DELETE /admin/users/:id`

Permanently deletes the user and cascades all associated data. Returns 204 No Content. Admins cannot delete themselves.

#### Updated response types

`UserResponse` now includes a `status` field (`"active"` or `"disabled"`).

### Health Records

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/health-records` | List health records (paginated, filterable by type/source/date range) | 1 |
| POST | `/health-records` | Create a health record (manual entry) | 1 |
| GET | `/health-records/:id` | Get a single health record | 1 |
| DELETE | `/health-records/:id` | Delete a health record | 1 |

### Interventions

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/interventions` | List interventions (paginated, filterable) | 1 |
| POST | `/interventions` | Log an intervention | 1 |
| GET | `/interventions/:id` | Get a single intervention | 1 |
| PATCH | `/interventions/:id` | Update an intervention's fields | 1 |
| DELETE | `/interventions/:id` | Delete an intervention | 1 |

#### `PATCH /interventions/:id`

Updates any subset of an intervention's mutable fields. All fields are
optional — unset fields are left unchanged (`COALESCE` semantics), and
**there is no way to clear a field back to `null` via this endpoint** — an
explicit `null` in the request body is indistinguishable from an omitted
key. No substance-name validation is applied, per project rules.

`updated_at` is bumped on every call, including a no-op `{}` body — the
response and stored row always reflect "last called", not "last changed".

**Request body:**

```json
{
  "substance": "caffeine",
  "dose": 200.0,
  "unit": "mg",
  "route": "oral",
  "administered_at": "2026-04-03T07:30:00Z",
  "fasted": true,
  "timing_relative_to": "pre-workout",
  "notes": "updated after re-reading the label"
}
```

**Response:** `200 OK` — the full updated intervention row, including the
`updated_at` timestamp.

**Errors:** `400` if `substance` is provided but blank. `404` if the
intervention doesn't exist or isn't owned by the caller.

### Daily Check-ins

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/checkins` | List check-ins (paginated) | 1 |
| POST | `/checkins` | Create a check-in (multiple per day allowed) | 1 |
| PUT | `/checkins/:id` | Update an existing check-in | 1 |
| GET | `/checkins/:id` | Get a single check-in | 1 |
| DELETE | `/checkins/:id` | Delete a check-in | 1 |

### Observations

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/observations` | List observations (filterable by type/name/date range) | 1 |
| POST | `/observations` | Create an observation | 1 |
| GET | `/observations/:id` | Get a single observation | 1 |
| DELETE | `/observations/:id` | Delete an observation | 1 |

### Lab Results

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/labs` | List lab results (paginated) | 1 |
| POST | `/labs` | Add a lab result | 1 |
| GET | `/labs/:id` | Get a single lab result | 1 |
| DELETE | `/labs/:id` | Delete a lab result | 1 |

### HealthKit Sync

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/healthkit/sync` | Bulk insert HealthKit records from iOS | 1 |
| GET | `/healthkit/write-queue` | Get pending HealthKit write-back items for iOS | 1 |
| POST | `/healthkit/confirm` | Confirm HealthKit write-backs were completed, and/or report failed writes | 1 |

`GET /healthkit/write-queue` returns the caller's pending items — rows in
`healthkit_write_queue` with `confirmed_at IS NULL AND failed_at IS NULL`, oldest
first, capped at 100:

```json
[
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
]
```

Every field of `value` except `start_time` is nullable — the underlying `health_records.value`/`unit`/`end_time` columns are all optional, and a record posted without them still enqueues, with those keys present and `null` (never omitted). Clients must decode `value`/`unit`/`end_time` as optional and treat a null `value` as a fail-reportable item rather than a decode error.

`POST /healthkit/confirm` accepts:

```json
{
  "ids": ["77777777-7777-7777-7777-777777777777"],
  "failures": [
    { "id": "88888888-8888-8888-8888-888888888888", "error": "HealthKit authorization denied" }
  ]
}
```

- `ids` — items the client successfully wrote to HealthKit; their `confirmed_at` is set.
- `failures` — items the client attempted but could not write; their `failed_at` and `error` are set (`error` is truncated to 500 characters, on Unicode scalar boundaries). Optional and defaults to empty — older clients that only ever sent `ids` continue to work unchanged.
- Both `ids` and `failures` are scoped to the caller's own rows — a user cannot confirm or fail another user's queue items.
- Both updates run in a single transaction. If an id appears in both `ids` and `failures` in the same request, **confirm wins** (`confirmed_at` is set, `failed_at` is not). A row already marked failed by an earlier request is **not** re-confirmed by a later request that lists its id in `ids` — `confirm`'s guard excludes rows with `failed_at` already set, so the first terminal state (confirmed or failed) for a given row sticks.
- Duplicate ids within `failures` in one request are deduplicated before the update — the last occurrence in the array wins deterministically.
- Marking an item failed also removes it from the pending set returned by `GET /healthkit/write-queue`, same as confirming it — this matters because the 100-row cap orders by `scheduled_at ASC`, so a permanently-unwritable item that is never reported as failed would otherwise block every item behind it indefinitely. Only deterministic failures should be reported this way — a client should keep transient errors (e.g. a momentary `HKHealthStore.save()` failure) pending so they retry on the next poll, rather than retiring them.
- Responds `204 No Content` on success (whether or not any ids/failures were provided).

**Compatibility matrix:**

| iOS client | Backend | Result |
|---|---|---|
| old (sends `{"ids": [...]}` only) | new (this change) | Works unchanged — `failures` defaults to empty. |
| new (sends `failures`) | old (pre-this-change) | The old server's JSON deserializer silently ignores the unrecognized `failures` field (no `deny_unknown_fields`) — the request still 204s, but nothing is recorded for the failed items. They are neither confirmed nor retired, and stay pending indefinitely. Self-hosters: upgrade the backend before or alongside an iOS build that reports failures. |

The Pact consumer contract (`pact/contracts/ios-backend.json`) is ahead of the currently-shipped iOS client — it documents the `failures` field before the iOS PR that populates it has landed.

### Source Preferences

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/source-preferences` | List source preferences | 1 |
| POST | `/source-preferences` | Set source preferences | 1 |
| GET | `/sources/overlap-scan` | Scan the last 30 days for metrics recorded by more than one source | 1 |

The overlap scan drives the source-preference wizard. It returns, per metric
that has records from more than one source over the window, the competing
sources ordered by descending record count:

```json
{
  "metrics": [
    {
      "metric_type": "heart_rate",
      "sources": [
        { "source": "garmin", "record_count": 120 },
        { "source": "oura", "record_count": 95 }
      ]
    }
  ]
}
```

Metrics with only one source are omitted. The user resolves each conflict by
writing a preference via `POST /source-preferences`.

### Integrations

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/integrations` | List connected integrations | 1 |
| DELETE | `/integrations/:source` | Disconnect an integration | 1 |
| GET | `/auth/garmin/login` | Start the Garmin OAuth 1.0a flow (requires JWT) | 1 |
| GET | `/auth/garmin/callback` | Garmin OAuth 1.0a callback — exchanges and stores the token (requires JWT) | 1 |
| GET | `/auth/oura/login` | Start the Oura OAuth 2.0 flow (requires JWT) | 1 |
| GET | `/auth/oura/callback` | Oura OAuth 2.0 callback — exchanges and stores the token (requires JWT) | 1 |
| POST | `/integrations/mychart/connect` | Connect a MyChart / SMART-on-FHIR provider | 2 |
| POST | `/integrations/mychart/sync` | Import lab results from a connected MyChart provider | 2 |

#### MyChart / SMART-on-FHIR lab import

MyChart (Epic) and compatible patient portals expose lab data over the
[SMART-on-FHIR](https://hl7.org/fhir/smart-app-launch/) standard: an OAuth 2.0
authorization-code flow with PKCE, followed by a FHIR R4 REST API. Because the
authorization endpoint and FHIR base URL differ per healthcare provider, the
client performs the SMART launch and in-app authorization redirect, captures
the authorization `code`, then posts it to the backend together with the
discovered endpoints. The backend exchanges the code (PKCE, no client secret),
stores the tokens encrypted, and persists the non-secret FHIR connection
metadata for later syncs.

`POST /integrations/mychart/connect`

```json
{
  "fhir_base_url": "https://fhir.example.org/r4",
  "token_endpoint": "https://fhir.example.org/oauth2/token",
  "code": "auth-code-abc",
  "redirect_uri": "ownpulse://mychart-callback",
  "code_verifier": "pkce-verifier-xyz"
}
```

Response `200`:

```json
{ "source": "mychart", "connected": true }
```

`POST /integrations/mychart/sync` (empty body) imports laboratory `Observation`
resources into `lab_results`. Imports are idempotent — each row carries the
FHIR resource id as `source_id` and is deduplicated on re-sync.

Response `200`:

```json
{ "source": "mychart", "imported": 2, "skipped": 0 }
```

Lab data is health data: it is imported verbatim. Marker names and values are
never validated, filtered, or judged; out-of-range flags are derived only from
the provider-supplied reference range. Requires `MYCHART_CLIENT_ID` to be set
on the server.

Because `fhir_base_url` and `token_endpoint` are client-supplied URLs that the
server connects to directly, they pass a layered SSRF guard before any outbound
request, at both connect time and on every sync (the stored URLs are
re-validated, never trusted blindly):

- the scheme must be `https`;
- IP-literal hosts in private / loopback / link-local / CGNAT / multicast
  ranges are rejected, with IPv4-mapped IPv6 (`::ffff:169.254.169.254`)
  canonicalised to IPv4 first;
- obfuscated numeric hosts (decimal, hex, octal, leading-zero octets) are
  rejected;
- the outbound HTTP client follows **no redirects** (a valid host cannot bounce
  the server to an internal address); and
- a custom DNS resolver rejects any hostname that **resolves** to an internal
  address, closing DNS-rebinding.

Set `MYCHART_ALLOW_INSECURE_URLS=true` only for local development against a
non-HTTPS test server; the API refuses to start with it enabled outside
localhost.

### Export

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/export/json` | Full JSON export (streaming) | 1 |
| GET | `/export/csv` | Full CSV export (streaming) | 1 |

`/export/json` covers `health_records`, `interventions`, `daily_checkins`, `lab_results`, `observations` (which includes sleep and all other user-defined data), `protocols`, `protocol_lines`, `protocol_runs`, `protocol_doses`, and (only if present) `genetic_records`. **`/export/csv` covers `health_records` only** — it does not include interventions, checkins, labs, observations, protocols, or genetics; use `/export/json` for a complete export.

### Account

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/account` | Get account info | 1 |
| DELETE | `/account` | Delete account and anonymize all data (72h) | 1 |

### Friend Sharing

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/friends/shares` | Create a new share (direct or invite link) | 1 |
| GET | `/friends/shares/outgoing` | List shares you have created | 1 |
| GET | `/friends/shares/incoming` | List shares others have with you | 1 |
| POST | `/friends/shares/accept-link` | Accept a share via invite token | 1 |
| POST | `/friends/shares/:id/accept` | Accept a direct share | 1 |
| DELETE | `/friends/shares/:id` | Revoke (owner) or decline (friend) a share | 1 |
| PATCH | `/friends/shares/:id/permissions` | Update data type permissions (owner only) | 1 |
| GET | `/friends/:friend_id/data` | Get a friend's shared data | 1 |

#### POST `/friends/shares`

Create a new friend share. If `friend_email` is provided, the share is sent directly to that user. If omitted, an invite link is generated instead.

**Request body:**

```json
{
  "friend_email": "friend@example.com",
  "data_types": ["checkins", "health_records"]
}
```

- `friend_email` — optional; omit to create a link share with an invite token.
- `data_types` — required, non-empty. Valid values: `checkins`, `health_records`, `interventions`, `observations`, `lab_results`.

**Response:** `201 Created`

```json
{
  "id": "uuid",
  "owner_id": "uuid",
  "owner_email": "owner@example.com",
  "friend_id": "uuid or null",
  "friend_email": "friend@example.com or null",
  "status": "pending",
  "invite_token": "uuid-token or null",
  "data_types": ["checkins", "health_records"],
  "created_at": "2026-03-21T00:00:00Z",
  "accepted_at": null
}
```

- For direct shares, `friend_id` and `friend_email` are set; `invite_token` is null.
- For link shares, `friend_id` and `friend_email` are null; `invite_token` is set.
- Invite tokens expire after 7 days.

**Errors:** `400` if `data_types` is empty or contains invalid types. `400` if sharing with yourself.

#### GET `/friends/shares/outgoing`

List shares you have created. Excludes revoked and declined shares.

**Response:** `200 OK` — array of share objects (same shape as create response). The `invite_token` is included for link shares you own.

#### GET `/friends/shares/incoming`

List shares others have created with you. Excludes revoked and declined shares.

**Response:** `200 OK` — array of share objects.

- `invite_token` is always stripped (not visible to recipients).
- `owner_email` is masked (e.g., `t***@gmail.com`) for non-accepted shares to prevent email enumeration.

#### POST `/friends/shares/:id/accept`

Accept a pending direct share. Only the designated friend (the user whose `friend_id` matches the share) can accept.

**Response:** `204 No Content`

**Errors:** `404` if the share does not exist, is not pending, or the caller is not the designated friend. Link shares cannot be accepted via this endpoint.

#### POST `/friends/shares/accept-link`

Accept a share via invite token. Used for link shares (where no specific friend was designated).

**Request body:**

```json
{
  "token": "invite-token-uuid"
}
```

**Response:** `200 OK`

```json
{
  "id": "uuid",
  "owner_id": "uuid",
  "status": "accepted",
  "accepted_at": "2026-03-21T00:00:00Z"
}
```

- The invite token is NULLed after acceptance (single-use).
- Expired tokens (older than 7 days) are rejected.
- The owner cannot accept their own share link.

**Errors:** `404` if the token is invalid, expired, or already used.

#### DELETE `/friends/shares/:id`

Revoke or decline a share. If the caller is the owner, status is set to `revoked`. If the caller is the friend, status is set to `declined`.

**Response:** `204 No Content`

**Errors:** `404` if the share does not exist, the caller is neither owner nor friend, or the share is already revoked/declined.

#### PATCH `/friends/shares/:id/permissions`

Update the data types shared on an existing share. Owner only.

**Request body:**

```json
{
  "data_types": ["checkins", "observations"]
}
```

**Response:** `204 No Content`

**Errors:** `400` if `data_types` is empty or contains invalid types. `403` if the caller is not the share owner.

#### GET `/friends/:friend_id/data`

Retrieve shared data from a friend. The `friend_id` path parameter is the data owner's user ID. Only data types permitted by an accepted share are returned.

**Response:** `200 OK`

```json
{
  "checkins": [...],
  "health_records": [...],
  "observations": [...]
}
```

Only keys for permitted data types are included. Possible keys: `checkins`, `health_records`, `interventions`, `observations`, `lab_results`.

**Errors:** `403` if there is no accepted share granting access to any data types.

### Explore

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/explore/metrics` | List available metrics grouped by source | 1 |
| GET | `/explore/series` | Fetch a single time series with aggregation | 1 |
| POST | `/explore/series` | Batch fetch multiple time series | 1 |
| POST | `/explore/charts` | Save a chart configuration | 1 |
| GET | `/explore/charts` | List saved charts | 1 |
| GET | `/explore/charts/:id` | Get a saved chart by ID | 1 |
| PUT | `/explore/charts/:id` | Update a saved chart | 1 |
| DELETE | `/explore/charts/:id` | Delete a saved chart | 1 |

#### `GET /explore/metrics`

List all metric sources and fields available for the authenticated user. Lab markers are dynamically populated from the user's existing lab results.

**Response:** `200 OK`

```json
{
  "sources": [
    {
      "source": "health_records",
      "label": "Health Records",
      "metrics": [
        { "field": "heart_rate", "label": "Heart Rate", "unit": "bpm" },
        { "field": "heart_rate_variability", "label": "Heart Rate Variability", "unit": "ms" }
      ]
    },
    {
      "source": "checkins",
      "label": "Check-ins",
      "metrics": [
        { "field": "energy", "label": "Energy", "unit": "score" }
      ]
    },
    {
      "source": "labs",
      "label": "Lab Results",
      "metrics": [
        { "field": "testosterone", "label": "testosterone", "unit": "value" }
      ]
    },
    {
      "source": "calendar",
      "label": "Calendar",
      "metrics": [
        { "field": "meeting_minutes", "label": "Meeting Minutes", "unit": "min" }
      ]
    },
    {
      "source": "sleep",
      "label": "Sleep",
      "metrics": [
        { "field": "duration_minutes", "label": "Sleep Duration", "unit": "min" }
      ]
    }
  ]
}
```

**Metric sources and fields:**

| Source | Fields |
|--------|--------|
| `health_records` | `heart_rate`, `heart_rate_variability`, `resting_heart_rate`, `body_mass`, `body_fat_percentage`, `body_temperature`, `blood_pressure_systolic`, `blood_pressure_diastolic`, `blood_glucose`, `blood_oxygen`, `respiratory_rate`, `steps`, `active_energy`, `basal_energy`, `vo2_max` |
| `checkins` | `energy`, `mood`, `focus`, `recovery`, `libido` |
| `labs` | Dynamic — any lab test name the user has recorded |
| `calendar` | `meeting_minutes`, `meeting_count` |
| `sleep` | `duration_minutes`, `deep_minutes`, `rem_minutes`, `score` |

#### `GET /explore/series`

Fetch a single time series with aggregation.

**Query parameters:**

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `source` | string | yes | Metric source (e.g. `health_records`, `checkins`) |
| `field` | string | yes | Metric field (e.g. `heart_rate`, `energy`) |
| `start` | ISO 8601 datetime | yes | Start of date range |
| `end` | ISO 8601 datetime | yes | End of date range |
| `resolution` | string | yes | `daily`, `weekly`, or `monthly` |

**Response:** `200 OK`

```json
{
  "source": "health_records",
  "field": "heart_rate",
  "unit": "bpm",
  "points": [
    { "t": "2026-03-01T00:00:00Z", "v": 62.5, "n": 24 },
    { "t": "2026-03-02T00:00:00Z", "v": 64.1, "n": 18 }
  ]
}
```

Each point contains: `t` (bucket timestamp), `v` (average value), `n` (number of raw records in the bucket).

**Errors:** `400` if source or field is invalid.

#### `POST /explore/series`

Batch fetch multiple time series in a single request. Queries run in parallel on the server.

**Request body:**

```json
{
  "metrics": [
    { "source": "health_records", "field": "heart_rate" },
    { "source": "checkins", "field": "energy" }
  ],
  "start": "2026-01-01T00:00:00Z",
  "end": "2026-03-28T00:00:00Z",
  "resolution": "daily"
}
```

- `metrics` — 1 to 8 metric specs.

**Response:** `200 OK`

```json
{
  "series": [
    {
      "source": "health_records",
      "field": "heart_rate",
      "unit": "bpm",
      "points": [...]
    },
    {
      "source": "checkins",
      "field": "energy",
      "unit": "score",
      "points": [...]
    }
  ]
}
```

**Errors:** `400` if `metrics` is empty, has more than 8 items, or contains invalid source/field combinations.

#### `POST /explore/charts`

Save a chart configuration.

**Request body:**

```json
{
  "name": "Morning vitals",
  "config": {
    "version": 1,
    "metrics": [
      { "source": "health_records", "field": "heart_rate", "color": "#ff0000" },
      { "source": "checkins", "field": "energy" }
    ],
    "range": { "preset": "30d" },
    "resolution": "daily"
  }
}
```

- `name` — 1 to 200 characters.
- `config.version` — must be `1`.
- `config.metrics` — 1 to 8 metrics. `color` is optional (`#rrggbb` format).
- `config.range` — either `{"preset": "7d|30d|90d|1y|all"}` or `{"start": "YYYY-MM-DD", "end": "YYYY-MM-DD"}`.
- `config.resolution` — `daily`, `weekly`, or `monthly`.

**Response:** `201 Created` — `ChartRow`

```json
{
  "id": "uuid",
  "user_id": "uuid",
  "name": "Morning vitals",
  "config": { ... },
  "created_at": "2026-03-28T00:00:00Z",
  "updated_at": "2026-03-28T00:00:00Z"
}
```

**Errors:** `400` if name is empty/too long, config version is unsupported, metrics are invalid, or range preset is unknown.

#### `GET /explore/charts`

List all saved charts for the authenticated user.

**Response:** `200 OK` — `ChartRow[]`

#### `GET /explore/charts/:id`

Get a saved chart by ID. Only the owner can access their charts.

**Response:** `200 OK` — `ChartRow`

**Errors:** `404` if chart not found or not owned by user.

#### `PUT /explore/charts/:id`

Update a saved chart's name and/or config. Both fields are optional — only provided fields are updated.

**Request body:**

```json
{
  "name": "Updated name",
  "config": { ... }
}
```

**Response:** `200 OK` — `ChartRow` (updated)

**Errors:** `404` if chart not found. `400` if config is invalid.

#### `DELETE /explore/charts/:id`

Delete a saved chart. Returns `204 No Content` on success, `404` if not found.

### Observer Polls

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/observer-polls` | Create a poll | 1 |
| GET | `/observer-polls` | List polls owned by user | 1 |
| GET | `/observer-polls/:id` | Get poll detail with members | 1 |
| PATCH | `/observer-polls/:id` | Update poll name/prompt | 1 |
| DELETE | `/observer-polls/:id` | Soft-delete poll | 1 |
| POST | `/observer-polls/:id/invite` | Generate invite link | 1 |
| POST | `/observer-polls/accept` | Accept invite | 1 |
| GET | `/observer-polls/my-polls` | List polls where caller is observer | 1 |
| PUT | `/observer-polls/:id/respond` | Submit daily scores | 1 |
| GET | `/observer-polls/:id/responses` | Owner views responses | 1 |
| GET | `/observer-polls/:id/my-responses` | Observer views own responses | 1 |
| DELETE | `/observer-polls/responses/:id` | Observer deletes own response | 1 |
| GET | `/observer-polls/export` | Observer exports all responses | 1 |

#### `POST /observer-polls`

Create a new observer poll.

**Request body:**

```json
{
  "name": "Daily wellbeing check",
  "custom_prompt": "Rate how Tony seems today",
  "dimensions": ["energy", "mood", "focus"]
}
```

- `name` — 1 to 100 characters.
- `custom_prompt` — optional, max 500 characters. HTML tags are stripped.
- `dimensions` — 1 to 10 items. Each must be 1-50 alphanumeric/underscore characters.

**Response:** `201 Created`

```json
{
  "id": "uuid",
  "name": "Daily wellbeing check",
  "custom_prompt": "Rate how Tony seems today",
  "dimensions": ["energy", "mood", "focus"],
  "members": [],
  "created_at": "2026-03-28T00:00:00Z",
  "deleted_at": null
}
```

**Errors:** `400` for validation failures (empty name, too many dimensions, invalid dimension characters, prompt too long).

#### `GET /observer-polls`

List all polls owned by the authenticated user (excludes soft-deleted polls).

**Response:** `200 OK` — array of `PollResponse` (members array is empty in list view).

#### `GET /observer-polls/:id`

Get poll detail with members. Only the poll owner can access this. Observer emails are masked (e.g., `t***@example.com`).

**Response:** `200 OK`

```json
{
  "id": "uuid",
  "name": "Daily wellbeing check",
  "custom_prompt": "Rate how Tony seems today",
  "dimensions": ["energy", "mood", "focus"],
  "members": [
    {
      "id": "uuid",
      "observer_email": "j***@example.com",
      "accepted_at": "2026-03-28T00:00:00Z",
      "created_at": "2026-03-27T00:00:00Z"
    }
  ],
  "created_at": "2026-03-28T00:00:00Z",
  "deleted_at": null
}
```

**Errors:** `404` if poll not found or not owned by user.

#### `PATCH /observer-polls/:id`

Update poll name and/or custom prompt. Only the owner can update.

**Request body:**

```json
{
  "name": "Updated name",
  "custom_prompt": "Updated prompt"
}
```

Both fields are optional. HTML tags in `custom_prompt` are stripped.

**Response:** `200 OK` — `PollResponse` (members array is empty).

**Errors:** `404` if not found. `400` if name is empty/too long or prompt exceeds 500 characters.

#### `DELETE /observer-polls/:id`

Soft-delete a poll (sets `deleted_at`). Only the owner can delete.

**Response:** `204 No Content`

**Errors:** `404` if not found or not owned by user.

#### `POST /observer-polls/:id/invite`

Generate an invite link for the poll. The invite token is a UUID valid for 7 days. Only the poll owner can generate invites.

**Response:** `201 Created`

```json
{
  "invite_token": "uuid",
  "invite_expires_at": "2026-04-04T00:00:00Z",
  "invite_url": "https://app.ownpulse.health/observer-polls/accept?token=uuid"
}
```

**Errors:** `404` if poll not found or not owned by user.

#### `POST /observer-polls/accept`

Accept an observer poll invite. The response is uniform regardless of whether the token was valid, expired, or already used — this prevents token enumeration.

**Request body:**

```json
{
  "token": "uuid"
}
```

**Response:** `200 OK`

```json
{
  "status": "accepted"
}
```

If the token is invalid or expired, the response is still `200 OK` with `{"status": "acknowledged"}`.

#### `GET /observer-polls/my-polls`

List polls where the caller is an accepted observer. The poll owner's email is masked.

**Response:** `200 OK`

```json
[
  {
    "id": "uuid",
    "owner_display": "t***@example.com",
    "name": "Daily wellbeing check",
    "custom_prompt": "Rate how Tony seems today",
    "dimensions": ["energy", "mood", "focus"]
  }
]
```

#### `PUT /observer-polls/:id/respond`

Submit daily scores for a poll. The caller must be an accepted member. Scores are upserted — submitting for the same date replaces previous scores.

**Request body:**

```json
{
  "date": "2026-03-28",
  "scores": {
    "energy": 7,
    "mood": 8,
    "focus": 6
  }
}
```

- `date` — cannot be in the future.
- `scores` — must contain exactly the poll's dimensions, each with an integer value from 1 to 10.

**Response:** `201 Created` (new) or `200 OK` (updated) — the response row.

```json
{
  "id": "uuid",
  "poll_id": "uuid",
  "member_id": "uuid",
  "date": "2026-03-28",
  "scores": { "energy": 7, "mood": 8, "focus": 6 },
  "created_at": "2026-03-28T00:00:00Z"
}
```

**Errors:** `403` if caller is not an accepted member. `400` if scores are invalid or date is in the future.

#### `GET /observer-polls/:id/responses`

Owner views all responses for a poll. Observer emails are masked.

**Query parameters:**

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `start` | date (YYYY-MM-DD) | no | Filter responses from this date |
| `end` | date (YYYY-MM-DD) | no | Filter responses up to this date |

**Response:** `200 OK`

```json
{
  "responses": [
    {
      "id": "uuid",
      "member_id": "uuid",
      "observer_email": "j***@example.com",
      "date": "2026-03-28",
      "scores": { "energy": 7, "mood": 8, "focus": 6 },
      "created_at": "2026-03-28T00:00:00Z"
    }
  ]
}
```

**Errors:** `404` if poll not found or not owned by user.

#### `GET /observer-polls/:id/my-responses`

Observer views their own responses for a poll.

**Response:** `200 OK`

```json
{
  "responses": [
    {
      "id": "uuid",
      "date": "2026-03-28",
      "scores": { "energy": 7, "mood": 8, "focus": 6 },
      "created_at": "2026-03-28T00:00:00Z"
    }
  ]
}
```

**Errors:** `403` if caller is not an accepted member of the poll.

#### `DELETE /observer-polls/responses/:id`

Observer deletes their own response. Only the observer who submitted the response can delete it.

**Response:** `204 No Content`

**Errors:** `404` if response not found or not owned by caller.

#### `GET /observer-polls/export`

Observer exports all their responses across all polls.

**Response:** `200 OK`

```json
{
  "responses": [
    {
      "id": "uuid",
      "poll_name": "Daily wellbeing check",
      "date": "2026-03-28",
      "scores": { "energy": 7, "mood": 8, "focus": 6 },
      "created_at": "2026-03-28T00:00:00Z"
    }
  ]
}
```

### Protocols

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/protocols` | Create protocol with lines | 1 |
| GET | `/protocols` | List user's protocols | 1 |
| GET | `/protocols/:id` | Get protocol with lines + dose status | 1 |
| PATCH | `/protocols/:id` | Update protocol | 1 |
| DELETE | `/protocols/:id` | Delete protocol | 1 |
| POST | `/protocols/:id/doses/log` | Log a dose directly on a protocol (legacy; resolves the protocol's current run) | 1 |
| POST | `/protocols/:id/doses/skip` | Skip a dose directly on a protocol (legacy; resolves the protocol's current run) | 1 |
| POST | `/protocols/runs/:run_id/doses/log` | Log a dose on an active run | 1 |
| POST | `/protocols/runs/:run_id/doses/skip` | Skip a dose on an active run | 1 |
| DELETE | `/protocols/runs/:run_id/doses/:dose_id` | Undo a logged/skipped dose on a run | 1 |
| GET | `/protocols/runs/todays-doses` | Today's scheduled doses across all of the user's active runs | 1 |
| POST | `/protocols/:id/share` | Generate share link | 1 |
| GET | `/protocols/shared/:token` | View shared protocol (public) | 1 |
| POST | `/protocols/import/:token` | Copy shared protocol | 1 |

#### `POST /protocols`

Create a new protocol with one or more lines and a day schedule.

**Request body:**

```json
{
  "name": "BPC-157 — 4 weeks",
  "start_date": "2026-04-01",
  "duration_days": 28,
  "lines": [
    {
      "substance": "BPC-157",
      "dose": "250 mcg",
      "route": "subcutaneous",
      "timing": "AM",
      "active_days": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28]
    }
  ]
}
```

- `duration_days` — total days in the protocol.
- `lines[].active_days` — array of 1-indexed day numbers within the duration when the dose is scheduled.

**Response:** `201 Created`

```json
{
  "id": "uuid",
  "name": "BPC-157 — 4 weeks",
  "start_date": "2026-04-01",
  "duration_days": 28,
  "lines": [
    {
      "id": "uuid",
      "substance": "BPC-157",
      "dose": "250 mcg",
      "route": "subcutaneous",
      "timing": "AM",
      "active_days": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28]
    }
  ],
  "created_at": "2026-03-27T00:00:00Z"
}
```

**Errors:** `400` if name is empty, duration is zero, or lines array is empty.

#### `POST /protocols/:id/doses/log` and `POST /protocols/runs/:run_id/doses/log`

Log a completed dose for a protocol line on a specific day of the protocol's
schedule (`day_number` is 0-indexed from the run's `start_date`). The
`:id` (protocol-level) form is the legacy path; the `:run_id` form operates
on a specific active run and is the one clients should use going forward.
Logging a dose also creates an `interventions` record for the line's
substance/dose/route.

The legacy `:id` form resolves the protocol's *current* run — its active
run, or its most recently created run if none is active — and writes to
that run (only a protocol with no runs at all writes a `NULL` run_id). This
means a dose logged through the legacy endpoint shows up correctly on the
run-scoped dose grid instead of being invisible on it, and a retry conflicts
(`409`) like it would on the `:run_id` form, rather than silently writing a
second, invisible row.

The dose grid on `GET /protocols/:id` (and on `GET /protocols/shared/:token`)
is scoped to that same single run — a second run of the same protocol no
longer shows the first run's checkmarks, and starts with an empty grid of
its own.

`administered_at` and `notes` are accepted and behave identically on both
the `:id` and `:run_id` forms (the legacy form delegates to the same
validation and timestamp logic as the run-scoped one).

**Request body:**

```json
{
  "protocol_line_id": "uuid",
  "day_number": 3,
  "administered_at": "2026-04-03T09:15:00Z",
  "notes": "logged a bit late",
  "tz_offset_minutes": -420
}
```

- `administered_at` — optional. Must fall within one calendar day of
  `start_date + day_number` (evaluated in `tz_offset_minutes` if given),
  otherwise `400`. When omitted, the created intervention's timestamp
  defaults to a time derived from the line's `time_of_day` — `AM` → `08:00`,
  `PM` → `20:00`, anything else → `12:00` — interpreted in
  `tz_offset_minutes` (UTC if omitted).
- `notes` — optional. Stored on the created intervention.
- `tz_offset_minutes` — optional, `-840`..`840` (UTC-14:00..UTC+14:00),
  otherwise `400`. The caller's local UTC offset, used both to resolve the
  default `administered_at` above and to evaluate date comparisons in the
  caller's own calendar day rather than UTC's. Defaults to UTC (`0`).

**Response:** `200 OK`

```json
{
  "id": "uuid",
  "protocol_line_id": "uuid",
  "day_number": 3,
  "status": "completed",
  "intervention_id": "uuid",
  "logged_at": "2026-04-03T08:30:00Z",
  "run_id": "uuid",
  "skip_reason": null
}
```

**Errors:** `404` if the protocol/run or line is not found or not owned by
the caller, or if `day_number` is out of range or not scheduled
(`schedule_pattern[day_number]` is `false`) for the line. `400` if
`day_number` is more than one day ahead of today (a single day of tolerance
absorbs timezone skew — a user east of UTC may legitimately be logging
"their today" while it's still tomorrow in UTC), if `administered_at` falls
more than a day from the calendar date for `day_number`, or if
`tz_offset_minutes` is out of range. `409` if a dose has already been logged
or skipped for this line, run, and day
(`UNIQUE(protocol_line_id, run_id, day_number)`). `422` if the request body
is missing `protocol_line_id` or `day_number`.

#### `POST /protocols/:id/doses/skip` and `POST /protocols/runs/:run_id/doses/skip`

Mark a scheduled dose as skipped, without creating an `interventions` record.
Same request body shape and error semantics as the log endpoints above,
except skips are allowed for any in-range day (past, present, or future) —
planned skips are legitimate — and there is no `administered_at`/`notes`/
`tz_offset_minutes` handling since no intervention is created. The legacy
`:id` form resolves the protocol's current run the same way the legacy log
endpoint does (see above).

**Request body:**

```json
{
  "protocol_line_id": "uuid",
  "day_number": 3,
  "skip_reason": "traveling, forgot supplies"
}
```

- `skip_reason` — optional free-text reason, stored on the dose row.

**Response:** `204 No Content`

#### `DELETE /protocols/runs/:run_id/doses/:dose_id`

Undo a logged or skipped dose: deletes the `protocol_doses` row and, if
logging it created one, the linked `interventions` row, in a single
transaction.

**Response:** `204 No Content`

**Errors:** `404` if the dose doesn't exist or doesn't belong to a run owned
by the caller.

#### `GET /protocols/runs/todays-doses`

Returns today's scheduled doses across all of the user's currently active
runs (paused or completed runs are excluded), one entry per protocol line
whose `schedule_pattern` marks today's day number as active.

**Response:** `200 OK`

```json
[
  {
    "protocol_id": "uuid",
    "protocol_name": "BPC-157 — 4 weeks",
    "run_id": "uuid",
    "protocol_line_id": "uuid",
    "substance": "BPC-157",
    "dose": 250.0,
    "unit": "mcg",
    "route": "subcutaneous",
    "time_of_day": "AM",
    "day_number": 3,
    "status": null
  }
]
```

`status` is `null` until a dose is logged or skipped for that line today, then
`"completed"` or `"skipped"`. `protocol_line_id` is the id to send back to the
log/skip endpoints above.

### Server-Sent Events (SSE)

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/events?token=<JWT>` | SSE stream for real-time data change notifications | 1 |

#### `GET /events?token=<JWT>`

Opens a Server-Sent Events stream for the authenticated user. Authentication is via the `token` query parameter because the browser `EventSource` API does not support custom headers.

The server sends `data_changed` events when the user's data is modified (e.g., new health records, check-ins, or sync completions). The connection includes a 30-second keepalive. The server re-validates the JWT and user status every 5 minutes, closing the stream if the token has expired or the user has been disabled.

**Event format:**

```
event: data_changed
data: {"source":"health_records","record_type":"heart_rate"}
```

- `source` — which data source changed (e.g. `health_records`, `checkins`, `interventions`).
- `record_type` — optional; the specific record type within the source.

**Errors:** `401` if the JWT is invalid. `403` if the user is disabled.

### Genetics

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/genetics/upload` | Upload a raw genetic data file (23andMe, AncestryDNA, VCF) | 2 |
| GET | `/genetics` | List genetic records with pagination, filterable by chromosome/rsid | 2 |
| GET | `/genetics/summary` | Summary counts (total variants, chromosomes, annotated count) | 2 |
| GET | `/genetics/interpretations` | User genotypes matched against the SNP annotation database | 2 |
| DELETE | `/genetics` | Delete all genetic records for the user (requires confirmation) | 2 |

Genetic records are stored in the `genetic_records` table (see
[data-model.md](data-model.md)) and have dedicated API endpoints, listed
above. Cooperative aggregation of genetic data still requires a separate
`sharing_consents` record with `dataset = 'genetics'` — that consent-gated
aggregation layer is design-only and not implemented (see
[Cooperative Sharing](#cooperative-sharing-phase-2) below).

#### `POST /genetics/upload`

Upload a genetic data file as `multipart/form-data`. Format (23andMe,
AncestryDNA, or VCF) is auto-detected from the file contents. Max file size
50 MB.

**Response:** `201 Created`

```json
{
  "total_variants": 638127,
  "new_variants": 638127,
  "duplicates_skipped": 0,
  "format": "23andme",
  "source": "23andme"
}
```

**Errors:** `400` if the file is empty, too large, an unrecognized format,
or contains no parseable variants.

#### `GET /genetics`

Query params: `page` (default 1), `per_page` (default 50, max 100),
`chromosome` (optional filter), `rsid` (optional filter).

**Response:** `200 OK`

```json
{
  "records": [
    {
      "id": "uuid",
      "user_id": "uuid",
      "source": "23andme",
      "rsid": "rs4988235",
      "chromosome": "2",
      "position": 136608646,
      "genotype": "AG",
      "uploaded_file_id": "uuid",
      "created_at": "2026-03-01T00:00:00Z"
    }
  ],
  "total": 638127,
  "page": 1,
  "per_page": 50
}
```

#### `GET /genetics/summary`

**Response:** `200 OK`

```json
{
  "total_variants": 638127,
  "source": "23andme",
  "uploaded_at": "2026-03-01T00:00:00Z",
  "chromosomes": { "1": 51234, "2": 48901 },
  "annotated_count": 412
}
```

#### `GET /genetics/interpretations`

Query params: `category` (optional filter). Joins the user's genotypes
against `snp_annotations` (ClinVar, PharmGKB, SNPedia). Every result cites
its source database and evidence level; the response always includes a
disclaimer that this is not medical advice.

**Response:** `200 OK`

```json
{
  "interpretations": [
    {
      "rsid": "rs4988235",
      "gene": "MCM6",
      "chromosome": "2",
      "position": 136608646,
      "user_genotype": "AG",
      "category": "metabolism",
      "title": "Lactase persistence",
      "summary": "...",
      "risk_level": "typical",
      "significance": "likely_benign",
      "evidence_level": "strong",
      "source": "SNPedia",
      "source_id": "rs4988235",
      "population_frequency": 0.74,
      "details": {}
    }
  ],
  "disclaimer": "This information is for educational purposes only and should not be used for medical decisions. Consult a healthcare provider or genetic counselor for clinical interpretation."
}
```

#### `DELETE /genetics`

**Request body:**

```json
{ "confirm": true }
```

**Response:** `204 No Content`. **Errors:** `400` if `confirm` is not `true`.

### Correlation / Stats

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/stats/correlate` | Pearson or Spearman correlation between two metrics over a time range | 3 |
| POST | `/stats/lag-correlate` | Correlation swept across a range of day lags to find the strongest offset | 3 |
| POST | `/stats/before-after` | Compare a metric's mean before vs. after an intervention's dose window (Welch's t-test) | 3 |

All three endpoints share a `MetricRef` shape for identifying a metric:
`{ "source": "string", "field": "string" }` (same source/field pairs as
`/explore/series`). `resolution` is **required** on all three requests (one
of `daily`, `weekly`, `monthly` — there is no default). `method` (where
present) is `pearson` (default) or `spearman`.

#### `POST /stats/correlate`

**Request body:**

```json
{
  "metric_a": { "source": "health_records", "field": "heart_rate_variability" },
  "metric_b": { "source": "checkins", "field": "energy" },
  "start": "2026-01-01T00:00:00Z",
  "end": "2026-03-01T00:00:00Z",
  "resolution": "daily",
  "method": "pearson"
}
```

**Response:** `200 OK`

```json
{
  "metric_a": { "source": "health_records", "field": "heart_rate_variability" },
  "metric_b": { "source": "checkins", "field": "energy" },
  "r": 0.42,
  "p_value": 0.01,
  "n": 58,
  "significant": true,
  "method": "pearson",
  "interpretation": "moderate positive correlation",
  "scatter": [{ "a": 55.2, "b": 7.0, "t": "2026-01-01T00:00:00Z" }]
}
```

Series are aligned by matching timestamp bucket; only buckets present in
both series are included. `scatter` always contains every aligned point
regardless of how many there are — it is not gated on the 3-point minimum.
`r` and `p_value` are `null` (present in the response, not omitted) when
fewer than 3 aligned points exist; `significant` is `false` in that case.

**Errors:** `400` if `start >= end` or a metric ref does not resolve to a
known source/field.

#### `POST /stats/lag-correlate`

Same request shape as `/stats/correlate` plus `max_lag_days` (integer,
1-30). Both series are fetched with an extra `max_lag_days` of margin
before `start` and after `end`, so a shift at the edge of the requested
range still has data to pair against. Sweeps lag from `-max_lag_days` to
`+max_lag_days`; for each lag `L`, metric A's value on day `d` is paired
with metric B's value on day `d + L`. A positive `L` therefore means A on
day `d` is compared against B `L` days later (A leads B by `L` days); a
negative `L` means A is compared against B from `L` days earlier (B leads
A). Returns one result per lag plus the lag with the strongest `|r|`.

**Request body:**

```json
{
  "metric_a": { "source": "health_records", "field": "heart_rate_variability" },
  "metric_b": { "source": "checkins", "field": "energy" },
  "start": "2026-01-01T00:00:00Z",
  "end": "2026-03-01T00:00:00Z",
  "resolution": "daily",
  "max_lag_days": 7,
  "method": "pearson"
}
```

**Response:** `200 OK`

```json
{
  "metric_a": { "source": "health_records", "field": "heart_rate_variability" },
  "metric_b": { "source": "checkins", "field": "energy" },
  "lags": [{ "lag": -1, "r": 0.31, "p_value": 0.04, "n": 40 }],
  "best_lag": { "lag": 2, "r": 0.51, "p_value": 0.002 },
  "method": "pearson"
}
```

`best_lag` is omitted from the response entirely (not `null`) when every
swept lag has fewer than 3 paired points or produces a `NaN` correlation —
i.e. no lag had a usable `r`.

**Errors:** `400` if `start >= end` or `max_lag_days` is out of range
(1-30).

#### `POST /stats/before-after`

Finds the first and last logged dose of `intervention_substance`, then
compares the metric's mean over a `before_days`-day window ending at the
first dose against an `after_days`-day window starting at the last dose
(or, if the intervention is still ongoing — last dose within 7 days of
now — from the first dose through now). Uses Welch's t-test. `first_dose`,
`last_dose`, `change_pct`, `p_value`, and `warning` are all optional
fields that are **omitted from the response entirely when absent** (not
serialized as `null`).

If either window has fewer than 3 points, `p_value` is omitted and
`significant` is `false`, but `change_pct` is still computed and included
when both window means exist; `warning` is included with the message
`"fewer than 3 data points in one or both windows — significance cannot
be determined"`.

**Request body:**

```json
{
  "intervention_substance": "Magnesium Glycinate",
  "metric": { "source": "checkins", "field": "energy" },
  "before_days": 14,
  "after_days": 14,
  "resolution": "daily"
}
```

**Response:** `200 OK`

```json
{
  "intervention_substance": "Magnesium Glycinate",
  "first_dose": "2026-02-01T08:00:00Z",
  "last_dose": "2026-02-01T08:00:00Z",
  "metric": { "source": "checkins", "field": "energy" },
  "before": { "mean": 6.1, "std_dev": 0.8, "n": 14, "points": [] },
  "after": { "mean": 7.3, "std_dev": 0.6, "n": 14, "points": [] },
  "change_pct": 19.7,
  "p_value": 0.02,
  "significant": true,
  "test_used": "welch_t"
}
```

If no interventions match `intervention_substance`, `first_dose`,
`last_dose`, `change_pct`, and `p_value` are all omitted, `before`/`after`
are both empty windows (`n: 0`), and `warning` is included with the
message `"no interventions found for this substance"`.

**Errors:** `400` if `intervention_substance` is empty, or `before_days`/
`after_days` is out of range (1-365).

## Planned — not implemented

The endpoints below do not exist yet — nothing to code against. They're kept
here as a record of intent, not a contract.

### Observations (Phase 2+)

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/observations/suggest` | Autocomplete observation names (from cooperative aggregate counts) | 2 |

### Export (Phase 2+)

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/export/fhir` | FHIR R4 export (streaming) | 2 |

### Cooperative Sharing (Phase 2+)

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/sharing/consents` | List sharing consents | 2 |
| POST | `/sharing/consents` | Grant sharing consent for a dataset | 2 |
| DELETE | `/sharing/consents/:dataset` | Revoke sharing consent (immediate) | 2 |
| POST | `/processing/restrict/:dataset` | Restrict processing without deletion | 2 |

The `sharing_consents` table exists, but there are no routes or aggregation
logic built on it yet — see [data-sharing.md](../cooperative/data-sharing.md)
for the designed-but-not-implemented cooperative aggregation layer. Genetic
data sharing has always required its own separate `dataset = 'genetics'`
consent record; that requirement carries over unchanged once this layer is
built. See [Genetics](#genetics) above for the (implemented) genetic data
endpoints themselves.
