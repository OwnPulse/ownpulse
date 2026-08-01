// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { useAuthStore } from "../store/auth";

/** Every source `publish_event` is called with in `backend/api/src/routes/`. */
export type BackendEventSource =
  | "health_records"
  | "protocols"
  | "interventions"
  | "checkins"
  | "labs"
  | "observations"
  | "genetics";

/**
 * Maps backend `data_changed` event sources to the TanStack Query keys that
 * should be invalidated. The backend uses snake_case source identifiers that
 * don't always match the kebab-case/plain query keys used by web components,
 * so a verbatim `[data.source]` invalidation silently misses queries (e.g.
 * `health_records` never matched the `["health-records"]` query key).
 * `insights` are server-generated from health records, check-ins, and
 * interventions, so those three also invalidate it.
 *
 * Sources not in this map fall back to invalidating `[source]` verbatim
 * (forward-compatible with a new backend source, though it won't match
 * anything until this map is updated — see the drift-guard test).
 */
export const SOURCE_QUERY_KEYS: Record<BackendEventSource, readonly string[]> = {
  health_records: ["health-records", "insights"],
  protocols: ["protocols", "todays-doses", "active-runs", "protocol-runs"],
  interventions: ["interventions", "todays-doses", "explore-interventions", "insights"],
  checkins: ["checkins", "insights"],
  labs: ["labs"],
  observations: ["observations"],
  genetics: ["genetics"],
};

function isBackendEventSource(source: string): source is BackendEventSource {
  return source in SOURCE_QUERY_KEYS;
}

export function useSSE() {
  const token = useAuthStore((s) => s.token);
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!token) return;

    const es = new EventSource(`/api/v1/events?token=${token}`);

    es.addEventListener("data_changed", (e) => {
      // The backend controls this payload, but a malformed or unexpected
      // shape must not throw inside the listener (EventSource has no error
      // boundary — an uncaught throw here would silently kill the stream).
      let parsed: unknown;
      try {
        parsed = JSON.parse(e.data);
      } catch {
        return;
      }
      if (
        typeof parsed !== "object" ||
        parsed === null ||
        typeof (parsed as { source?: unknown }).source !== "string"
      ) {
        return;
      }

      const source = (parsed as { source: string }).source;
      const keys = isBackendEventSource(source) ? SOURCE_QUERY_KEYS[source] : [source];
      for (const key of keys) {
        queryClient.invalidateQueries({ queryKey: [key] });
      }
      queryClient.invalidateQueries({ queryKey: ["explore-series"] });
      queryClient.invalidateQueries({ queryKey: ["dashboard-summary"] });
      queryClient.invalidateQueries({ queryKey: ["dashboard-sparklines"] });
    });

    es.onerror = () => {
      // EventSource auto-reconnects; no action needed
    };

    return () => es.close();
  }, [token, queryClient]);
}
