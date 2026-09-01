// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ApiError } from "../api/client";
import { integrationsApi, SYNCABLE_SOURCES } from "../api/integrations";
import styles from "./Sources.module.css";

// `GET /integrations` only lists sources the user has already connected — it
// isn't a catalog. Google Calendar is the only source with a working web
// connect flow so far (garmin/oura/mychart connect from the iOS app), so it's
// always shown as a row, connected or not.
const GOOGLE_CALENDAR_SOURCE = "google_calendar";
// A full browser navigation (not a `fetch`) can't carry the in-memory JWT as
// an Authorization header, so this only works today for users who still hold
// the short-lived `access_token` cookie the backend sets after a Google
// sign-in. Password-authenticated users hitting Connect will see the
// backend's 401 page — tracked as a follow-up to give this route the same
// `?token=` query-param fallback `/events` already has.
const GOOGLE_CALENDAR_LOGIN_URL = "/api/v1/auth/google-calendar/login";

export default function Sources() {
  const queryClient = useQueryClient();

  const integrations = useQuery({
    queryKey: ["integrations"],
    queryFn: () => integrationsApi.list(),
  });

  const disconnectMutation = useMutation({
    mutationFn: (source: string) => integrationsApi.disconnect(source),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["integrations"] });
    },
  });

  const syncMutation = useMutation({
    mutationFn: (source: string) => integrationsApi.sync(source),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["integrations"] });
    },
  });

  const syncErrorMessage = (source: string): string | null => {
    if (syncMutation.variables !== source || !syncMutation.isError) return null;
    const err = syncMutation.error;
    if (err instanceof ApiError && err.status === 429) {
      return err.retryAfterSeconds
        ? `Rate limited — try again in ${err.retryAfterSeconds}s.`
        : "Rate limited — try again shortly.";
    }
    return "Sync failed.";
  };

  const hasGoogleCalendar = integrations.data?.some((i) => i.source === GOOGLE_CALENDAR_SOURCE);

  return (
    <main className="op-page">
      <h1>Sources</h1>

      {integrations.isLoading && <p>Loading integrations...</p>}
      {integrations.isError && <p>Error loading integrations.</p>}
      {integrations.data && (
        <ul className={styles.integrationList}>
          {!hasGoogleCalendar && (
            <li className={styles.integrationItem}>
              <span className={styles.sourceName}>google_calendar</span>
              <span className={styles.statusDisconnected}>Disconnected</span>
              <a href={GOOGLE_CALENDAR_LOGIN_URL} className="op-btn op-btn-primary op-btn-sm">
                Connect
              </a>
            </li>
          )}
          {integrations.data.map((integration) => (
            <li key={integration.source} className={styles.integrationItem}>
              <span className={styles.sourceName}>{integration.source}</span>
              <span
                className={
                  integration.connected ? styles.statusConnected : styles.statusDisconnected
                }
              >
                {integration.connected ? "Connected" : "Disconnected"}
              </span>
              {integration.last_synced_at && (
                <span className={styles.syncTime}>Last sync: {integration.last_synced_at}</span>
              )}
              {integration.last_sync_error && (
                <span className={styles.syncError}>{integration.last_sync_error}</span>
              )}
              {integration.connected && SYNCABLE_SOURCES.has(integration.source) && (
                <button
                  type="button"
                  className="op-btn op-btn-secondary op-btn-sm"
                  onClick={() => syncMutation.mutate(integration.source)}
                  disabled={syncMutation.isPending && syncMutation.variables === integration.source}
                >
                  Sync now
                </button>
              )}
              {syncErrorMessage(integration.source) && (
                <span className={styles.syncError}>{syncErrorMessage(integration.source)}</span>
              )}
              {integration.connected && (
                <button
                  type="button"
                  className="op-btn op-btn-ghost op-btn-sm"
                  onClick={() => disconnectMutation.mutate(integration.source)}
                  disabled={disconnectMutation.isPending}
                >
                  Disconnect
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}
