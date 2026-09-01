// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ApiError } from "../api/client";
import { integrationsApi, SYNCABLE_SOURCES } from "../api/integrations";
import { QueryState } from "../components/QueryState";
import styles from "./Sources.module.css";

// `GET /integrations` only lists sources the user has already connected — it
// isn't a catalog. Google Calendar is the only source that will eventually
// have a working web connect flow (garmin/oura/mychart connect from the iOS
// app), so it's always shown as a row, connected or not.
const GOOGLE_CALENDAR_SOURCE = "google_calendar";

export default function Sources() {
  const queryClient = useQueryClient();

  const integrations = useQuery({
    queryKey: ["integrations"],
    queryFn: () => integrationsApi.list(),
  });

  const disconnectMutation = useMutation({
    mutationFn: (source: string) => integrationsApi.disconnect(source),
    onSuccess: () => {
      // A stale sync error/success from before this disconnect shouldn't
      // reappear if the same source gets reconnected later in this session.
      syncMutation.reset();
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

      <QueryState
        isLoading={integrations.isLoading}
        isFetching={integrations.isFetching}
        isError={integrations.isError}
        onRetry={() => integrations.refetch()}
        loadingText="Loading integrations..."
        errorText="Error loading integrations."
      >
        {integrations.data && (
          <ul className={styles.integrationList}>
            {!hasGoogleCalendar && (
              <li className={styles.integrationItem}>
                <span className={styles.sourceName}>google_calendar</span>
                <span className={styles.statusDisconnected}>Disconnected</span>
                {/* The connect flow needs a JWT to authorize `google_calendar_login`,
                    but a plain browser navigation (as opposed to `fetch`) can't carry
                    the in-memory JWT as an Authorization header — every user 401s on
                    this route today, including the OAuth callback leg. Re-enable this
                    once fix/calendar-connect-browser-nav lands the backend's `?token=`
                    query-param fallback (mirroring `/events`); the href will need
                    `?token=${accessToken}` appended at that point. */}
                <button
                  type="button"
                  className="op-btn op-btn-primary op-btn-sm"
                  disabled
                  title="Connecting from the web is coming soon"
                >
                  Connect
                </button>
                <span className={styles.connectHint}>Connecting from the web is coming soon</span>
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
                  <span className={styles.syncError} title={integration.last_sync_error}>
                    {integration.last_sync_error}
                  </span>
                )}
                {integration.connected && SYNCABLE_SOURCES.has(integration.source) && (
                  <button
                    type="button"
                    className="op-btn op-btn-secondary op-btn-sm"
                    onClick={() => syncMutation.mutate(integration.source)}
                    disabled={
                      syncMutation.isPending && syncMutation.variables === integration.source
                    }
                  >
                    Sync now
                  </button>
                )}
                {syncErrorMessage(integration.source) && (
                  <span
                    className={styles.syncError}
                    title={syncErrorMessage(integration.source) ?? undefined}
                  >
                    {syncErrorMessage(integration.source)}
                  </span>
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
      </QueryState>
    </main>
  );
}
