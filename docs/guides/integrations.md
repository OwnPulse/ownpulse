# Adding a New Integration

New data sources follow a consistent 12-step process. This ensures each integration has the same quality: OAuth flow, sync job, error handling, tests, and documentation.

## Steps

1. **OAuth token storage.** The `integration_tokens` table already handles this. No schema change needed unless the provider requires non-standard fields.

2. **Create the HTTP client.** Add `backend/api/src/integrations/<source>.rs`. Implement a client struct with methods for each API endpoint. The client must be WireMock-compatible (accept a base URL parameter).

3. **Record real API responses.** Call the real API once and save responses to `backend/tests/fixtures/<source>/`. These become the WireMock fixtures.

4. **Add WireMock stubs.** Update `backend/tests/common/mock_servers.rs` with stubs for the new source's API endpoints.

5. **Add the sync job.** Create `backend/api/src/jobs/<source>_sync.rs`. The job fetches data from the API, transforms it to the internal schema, and inserts into the appropriate tables. Handle pagination, rate limiting, and token refresh.

6. **Add OAuth routes.** Add the provider's OAuth initiation and callback to `backend/api/src/routes/auth.rs`.

7. **Add sync routes.** Add manual sync trigger and integration status to `backend/api/src/routes/integrations.rs`.

8. **Write integration tests.** Cover:
   - OAuth flow (initiation, callback, token storage)
   - Sync happy path (data fetched, transformed, inserted)
   - Sync error handling (API errors, rate limits, invalid tokens)
   - Token refresh flow

9. **Add WireMock fixtures.** Ensure all mocked responses are committed in `backend/tests/fixtures/<source>/`.

10. **Update documentation.** Add the new source to `docs/architecture/api.md` and any relevant guides.

11. **Add source connection UI.** Add the provider to `web/src/pages/Sources.tsx` with OAuth initiation button and connection status display.

12. **Add Playwright test.** Write an E2E test for the OAuth connection flow in `web/tests/e2e/`.

## Conventions

- Never hit real external APIs in tests. WireMock fixtures only.
- Store OAuth tokens encrypted with AES-256-GCM in `integration_tokens`.
- Each sync job is idempotent. Running it twice for the same time range produces the same result.
- Handle deduplication: if the new source also syncs to HealthKit, trigger the overlap scan and source preference wizard.

## Existing Integrations

| Provider | Status | ADR |
|----------|--------|-----|
| HealthKit | Phase 1 | [ADR-0008](../decisions/0008-healthkit-sync.md) |
| Garmin | Phase 1 | -- |
| Oura | Phase 1 | -- |
| Dexcom | Phase 2 | -- |
| Google Calendar | Phase 1 | -- |
