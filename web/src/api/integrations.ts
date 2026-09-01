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

// Sources with a `POST /integrations/<sync-path>/sync` route that returns the
// `{ source, records_inserted }` shape `SyncResult` below expects. MyChart
// has its own sync route too, but its response is `{ source, imported }` —
// a different shape — and isn't a fit for this generic Sources-page sync
// button; it stays iOS-only for now (see `Settings > Lab Results` in the
// app) rather than lying about its result count here.
export const SYNCABLE_SOURCES = new Set(["garmin", "oura", "google_calendar"]);

// The sync routes are fixed paths registered ahead of `/integrations/:source`
// (see `backend/api/src/routes/mod.rs`), and `google_calendar`'s is
// registered as `google-calendar` (hyphenated) even though `source` in
// `IntegrationStatus`/the disconnect path is the underscored `google_calendar`
// — the two identifiers just don't match here on the backend. Spelled out
// explicitly (rather than a blanket `_` -> `-` replace) so a future
// multi-word source with a genuinely underscored path doesn't silently
// get mangled.
const SYNC_PATH_OVERRIDES: Record<string, string> = {
  google_calendar: "google-calendar",
};

function syncPath(source: string): string {
  return SYNC_PATH_OVERRIDES[source] ?? source;
}

export const integrationsApi = {
  list: () => api.get<IntegrationStatus[]>("/api/v1/integrations"),
  disconnect: (source: string) => api.delete<void>(`/api/v1/integrations/${source}`),
  // `sync` takes no body — it's a trigger, not a data submission — but the
  // client's `post` always JSON-encodes its second argument, so pass `{}`
  // rather than adding a bodyless method just for this one call.
  sync: (source: string) =>
    api.post<SyncResult>(`/api/v1/integrations/${syncPath(source)}/sync`, {}),
};
