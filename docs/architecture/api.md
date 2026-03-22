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

#### `POST /auth/register`

Register a new account. When the instance requires invites (`REQUIRE_INVITE=true`, the default), a valid invite code must be provided.

**Request body:**

```json
{
  "username": "string",
  "password": "string",
  "invite_code": "string"
}
```

**Response:** `TokenResponse` (same shape as `/auth/login`).

**Errors:**

| Status | Reason |
|--------|--------|
| 400 | Invalid or expired invite code, or validation failure |
| 409 | Username already taken |

#### Google OAuth with invite code

`GET /auth/google/login` accepts an optional `?invite_code=XYZ` query parameter. If the user does not yet have an account and invite codes are required, the invite code is validated during the OAuth callback. If no valid code is present, the callback redirects to the login page with `?error=invite_required`.

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
