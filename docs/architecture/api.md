# API Reference

**Base URL:** `https://api.<domain>/api/v1`

All endpoints require JWT authentication unless marked as public. Tokens are issued via the auth endpoints and passed as `Authorization: Bearer <token>`.

## Implemented

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/health` | Health check (public) | 1 |

## Planned

### Auth

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/auth/login` | Login with email/password, returns JWT + refresh token | 1 |
| POST | `/auth/refresh` | Exchange refresh token for new JWT | 1 |
| POST | `/auth/logout` | Revoke refresh token | 1 |
| GET | `/auth/google/callback` | Google OAuth callback | 1 |
| GET | `/auth/garmin/callback` | Garmin OAuth callback | 1 |
| GET | `/auth/oura/callback` | Oura OAuth callback | 1 |
| GET | `/auth/dexcom/callback` | Dexcom OAuth callback | 2 |

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
| POST | `/checkins` | Submit a daily check-in | 1 |
| GET | `/checkins/:date` | Get check-in for a specific date | 1 |

### Observations

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/observations` | List observations (filterable by type/name/date range) | 1 |
| POST | `/observations` | Create an observation | 1 |
| GET | `/observations/:id` | Get a single observation | 1 |
| DELETE | `/observations/:id` | Delete an observation | 1 |
| GET | `/observations/suggest` | Autocomplete observation names (from cooperative aggregate counts) | 2 |

### Lab Results

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/labs` | List lab results (paginated) | 1 |
| POST | `/labs` | Add a lab result | 1 |
| GET | `/labs/:id` | Get a single lab result | 1 |
| DELETE | `/labs/:id` | Delete a lab result | 1 |

### Integrations

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/integrations` | List connected integrations | 1 |
| POST | `/integrations/:provider/connect` | Start OAuth flow for a provider | 1 |
| DELETE | `/integrations/:provider` | Disconnect an integration | 1 |
| POST | `/integrations/:provider/sync` | Trigger manual sync | 1 |
| GET | `/source-preferences` | List source preferences | 1 |
| PUT | `/source-preferences/:metric_type` | Set preferred source for a metric | 1 |

### HealthKit Sync

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/healthkit/records` | Bulk upload HealthKit records from iOS | 1 |
| GET | `/healthkit/write-queue` | Get pending write-back items for iOS | 1 |
| POST | `/healthkit/write-queue/:id/confirm` | Confirm a write-back was completed | 1 |

### Export

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/export/json` | Full JSON export (streaming) | 1 |
| GET | `/export/csv` | Full CSV export (streaming) | 1 |
| GET | `/export/fhir` | FHIR R4 export (streaming) | 2 |

### Cooperative Sharing

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/sharing/consents` | List sharing consents | 2 |
| POST | `/sharing/consents` | Grant sharing consent for a dataset | 2 |
| DELETE | `/sharing/consents/:dataset` | Revoke sharing consent (immediate) | 2 |
| POST | `/processing/restrict/:dataset` | Restrict processing without deletion | 2 |

### Correlation (Phase 3)

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| POST | `/stats/correlate` | Compute correlation between two metrics | 3 |
| POST | `/stats/lag-correlate` | Compute lag correlation | 3 |

### Account

| Method | Path | Description | Phase |
|--------|------|-------------|-------|
| GET | `/account` | Get account info | 1 |
| DELETE | `/account` | Delete account and anonymize all data (72h) | 1 |
