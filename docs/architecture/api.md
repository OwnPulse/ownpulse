# API Reference

**Base URL:** `https://api.<domain>/api/v1`

All endpoints require JWT authentication unless marked as public. Tokens are issued via the auth endpoints and passed as `Authorization: Bearer <token>`.

## Implemented

### Public

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/health` | Health check (public) | 1 |
| POST | `/waitlist` | Waitlist signup (public) | 1 |

### Auth (Public)

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/auth/login` | Login with username/password, returns JWT + refresh token | 1 |
| POST | `/auth/register` | Register with invite code (see below) | 1 |
| POST | `/auth/refresh` | Refresh token rotation (cookie) | 1 |
| POST | `/auth/logout` | Invalidate refresh token | 1 |
| GET | `/auth/google/login` | Google OAuth redirect (accepts optional `?invite_code=XYZ`) | 1 |
| GET | `/auth/google/callback` | Google OAuth callback | 1 |
| POST | `/auth/apple/callback` | Apple Sign-In callback (verify id_token, issue tokens) | 1 |
| GET | `/auth/methods` | List auth methods linked to current user (requires JWT) | 1 |
| POST | `/auth/link` | Link a new auth provider to current user (requires JWT) | 1 |
| DELETE | `/auth/link/:provider` | Unlink an auth provider from current user (requires JWT) | 1 |

#### `POST /auth/register`

Register a new account. When the instance requires invites (`REQUIRE_INVITE=true`, the default), a valid invite code must be provided.

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
| 400 | Missing required field for provider, password too short, unsupported provider, or Google linking (not yet supported) |
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
| DELETE | `/interventions/:id` | Delete an intervention | 1 |

### Daily Check-ins

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/checkins` | List check-ins (paginated) | 1 |
| POST | `/checkins` | Submit a daily check-in (upsert) | 1 |
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
| POST | `/healthkit/confirm` | Confirm HealthKit write-backs were completed | 1 |

### Source Preferences

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/source-preferences` | List source preferences | 1 |
| POST | `/source-preferences` | Set source preferences | 1 |

### Integrations

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/integrations` | List connected integrations | 1 |
| DELETE | `/integrations/:source` | Disconnect an integration | 1 |

### Export

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/export/json` | Full JSON export (streaming) | 1 |
| GET | `/export/csv` | Full CSV export (streaming) | 1 |

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

## Planned (Phase 2+)

### Auth (Phase 2+)

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/auth/garmin/callback` | Garmin OAuth callback | 2+ |
| GET | `/auth/oura/callback` | Oura OAuth callback | 2+ |
| GET | `/auth/dexcom/callback` | Dexcom OAuth callback | 2+ |

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

### Genetic Data (Phase 2+)

Genetic records are stored in the `genetic_records` table but do not yet have dedicated API endpoints. These will be added in Phase 2+ with separate sharing consent requirements.

### Correlation (Phase 3)

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/stats/correlate` | Compute correlation between two metrics | 3 |
| POST | `/stats/lag-correlate` | Compute lag correlation | 3 |
