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

// The backend's connect flow (see fix/calendar-connect-browser-nav) redirects
// back here with `?connected=<source>` on success or `?error=<code>` on
// failure — mirrors Settings.tsx's SETTINGS_MESSAGES pattern for the
// analogous `/settings?linked=...`/`?error=...` redirects.
const SOURCES_MESSAGES: Record<string, { type: "success" | "error"; text: string }> = {
  "connected=google_calendar": { type: "success", text: "Google Calendar connected." },
  "error=access_denied": {
    type: "error",
    text: "Google Calendar connection was cancelled.",
  },
  "error=auth_required": { type: "error", text: "Your session expired. Please log in again." },
};
// Any other `error=<code>` we don't have specific copy for yet still gets a
// friendly (non-JSON, non-blank) banner instead of silently doing nothing.
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
                {/* The connect flow needs a JWT to authorize `google_calendar_login`,
                    but a plain browser navigation (as opposed to `fetch`) can't carry
                    the in-memory JWT as an Authorization header — every user 401s on
                    this route today, including the OAuth callback leg. Re-enable once
                    fix/calendar-connect-browser-nav lands: the reworked backend flow
                    is `fetch` (with `Authorization: Bearer`) -> `{ auth_url }` JSON ->
                    `window.location = auth_url` — NOT a plain `<a href>` and NOT a
                    `?token=` query param, since the redirect itself doesn't need auth
                    once the backend hands back a ready-to-use `auth_url`. The success/
                    error landing (`?connected=google_calendar` / `?error=<code>`) is
                    already wired up above via SOURCES_MESSAGES. */}
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
