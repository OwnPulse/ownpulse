// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface IntegrationStatus {
  source: string;
  connected: boolean;
  last_synced_at?: string;
  last_sync_error?: string;
}

export interface SyncResult {
  source: string;
  records_inserted: number;
}

export interface GoogleCalendarAuthUrl {
  auth_url: string;
}

// Sources with a `POST /integrations/<source>/sync` route that returns the
// `{ source, records_inserted }` shape `SyncResult` below expects. MyChart
// has its own sync route too, but its response is `{ source, imported }` —
// a different shape — and isn't a fit for this generic Sources-page sync
// button; it stays iOS-only for now (see `Settings > Lab Results` in the
// app) rather than lying about its result count here.
export const SYNCABLE_SOURCES = new Set(["garmin", "oura", "google_calendar"]);

export const integrationsApi = {
  list: () => api.get<IntegrationStatus[]>("/api/v1/integrations"),
  disconnect: (source: string) => api.delete<void>(`/api/v1/integrations/${source}`),
  // `sync` takes no body — it's a trigger, not a data submission — but the
  // client's `post` always JSON-encodes its second argument, so pass `{}`
  // rather than adding a bodyless method just for this one call.
  // `google_calendar`'s sync route is also reachable at the historical
  // hyphenated `/integrations/google-calendar/sync` path, but the backend
  // aliases it at the underscored `source` id too (see
  // `backend/api/src/routes/mod.rs`), so building the URL directly from
  // `source` — same as `disconnect` above — needs no special-casing.
  sync: (source: string) => api.post<SyncResult>(`/api/v1/integrations/${source}/sync`, {}),
  // GET, not POST: starting the flow doesn't change any state on its own
  // (it only records a short-lived `oauth_states` row keyed by the CSRF
  // value embedded in the returned `auth_url`) — see docs/architecture/api.md.
  googleCalendarAuthUrl: () => api.get<GoogleCalendarAuthUrl>("/api/v1/auth/google-calendar/login"),
};
