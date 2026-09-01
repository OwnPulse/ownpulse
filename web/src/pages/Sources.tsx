// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { ApiError } from "../api/client";
import { integrationsApi, SYNCABLE_SOURCES } from "../api/integrations";
import { QueryState } from "../components/QueryState";
import styles from "./Sources.module.css";

// `GET /integrations` only lists sources the user has already connected — it
// isn't a catalog. Google Calendar is the only source that will eventually
// have a working web connect flow (garmin/oura/mychart connect from the iOS
// app), so it's always shown as a row, connected or not.
const GOOGLE_CALENDAR_SOURCE = "google_calendar";

// `/auth/google-calendar/callback` (see docs/architecture/api.md) redirects
// back here with `?connected=google_calendar` on success, or `?error=<code>`
// on failure using one of its five documented codes — mirrors Settings.tsx's
// SETTINGS_MESSAGES pattern for the analogous `/settings?linked=...`/
// `?error=...` redirects.
const SOURCES_MESSAGES: Record<string, { type: "success" | "error"; text: string }> = {
  "connected=google_calendar": { type: "success", text: "Google Calendar connected." },
  "error=access_denied": { type: "error", text: "Google Calendar connection was cancelled." },
  "error=state_invalid": {
    type: "error",
    text: "That connection link expired or was already used. Please try connecting again.",
  },
  "error=missing_code": {
    type: "error",
    text: "Google didn't return the information OwnPulse needed. Please try connecting again.",
  },
  "error=exchange_failed": {
    type: "error",
    text: "Google Calendar couldn't confirm the connection. Please try again.",
  },
  "error=server_error": {
    type: "error",
    text: "OwnPulse hit a problem connecting Google Calendar. Please try again.",
  },
};
// Any error code not in the map above (e.g. a future one added on the
// backend before this map is updated) still gets a friendly, non-blank
// banner instead of silently doing nothing.
const GENERIC_CONNECT_ERROR = "Couldn't connect Google Calendar. Please try again.";

export default function Sources() {
  const queryClient = useQueryClient();
  const [searchParams, setSearchParams] = useSearchParams();
  const [statusMsg, setStatusMsg] = useState<{ type: "success" | "error"; text: string } | null>(
    null,
  );

  useEffect(() => {
    const connected = searchParams.get("connected");
    const error = searchParams.get("error");

    let key: string | null = null;
    if (connected) key = `connected=${connected}`;
    else if (error) key = `error=${error}`;

    if (key && key in SOURCES_MESSAGES) {
      setStatusMsg(SOURCES_MESSAGES[key]);
      setSearchParams({}, { replace: true });
    } else if (error) {
      setStatusMsg({ type: "error", text: GENERIC_CONNECT_ERROR });
      setSearchParams({}, { replace: true });
    }
  }, [searchParams, setSearchParams]);

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

  // `googleCalendarAuthUrl` is a same-origin `fetch` with the normal Bearer
  // header (see `api/client.ts`) — the JWT never appears in a URL. The
  // backend only hands back a ready-to-use Google `auth_url` to navigate to;
  // it doesn't need to receive the JWT again on that navigation itself.
  const connectMutation = useMutation({
    mutationFn: () => integrationsApi.googleCalendarAuthUrl(),
    onSuccess: (data) => {
      window.location.href = data.auth_url;
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

      {statusMsg && (
        <p className={statusMsg.type === "error" ? "op-error-msg" : "op-success-msg"}>
          {statusMsg.text}
        </p>
      )}

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
                <button
                  type="button"
                  className="op-btn op-btn-primary op-btn-sm"
                  onClick={() => connectMutation.mutate()}
                  disabled={connectMutation.isPending}
                >
                  {connectMutation.isPending ? "Connecting..." : "Connect"}
                </button>
                {connectMutation.isError && (
                  <span className={styles.syncError}>
                    Couldn't start the Google Calendar connection. Please try again.
                  </span>
                )}
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
