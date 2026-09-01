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

/** Sources with a `POST /integrations/<sync-path>/sync` route wired up on the backend. */
export const SYNCABLE_SOURCES = new Set(["garmin", "oura", "google_calendar", "mychart"]);

// The sync routes are fixed paths registered ahead of `/integrations/:source`
// (see `backend/api/src/routes/mod.rs`), and `google_calendar`'s is
// registered as `google-calendar` (hyphenated) even though `source` in
// `IntegrationStatus`/the disconnect path is the underscored `google_calendar`
// — the two identifiers just don't match here on the backend.
function syncPath(source: string): string {
  return source.replace(/_/g, "-");
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
