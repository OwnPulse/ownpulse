// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { useAuthStore } from "../store/auth";

/**
 * Maps backend `data_changed` event sources (see `publish_event` call sites in
 * `backend/api/src/routes/`) to the TanStack Query keys that should be
 * invalidated. The backend uses snake_case source identifiers that don't
 * always match the kebab-case/plain query keys used by web components, so a
 * verbatim `[data.source]` invalidation silently misses queries (e.g.
 * `health_records` never matched the `["health-records"]` query key).
 *
 * Sources not listed here fall back to invalidating `[source]` verbatim —
 * forward-compatible with new backend sources, though it likely won't match
 * anything until this map is updated.
 */
const SOURCE_QUERY_KEYS: Record<string, string[]> = {
  health_records: ["health-records", "dashboard-sparklines"],
  protocols: ["protocols", "todays-doses", "active-runs", "protocol-runs"],
  interventions: ["interventions", "todays-doses"],
  checkins: ["checkins"],
  labs: ["labs"],
  observations: ["observations"],
  genetics: ["genetics"],
};

export function useSSE() {
  const token = useAuthStore((s) => s.token);
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!token) return;

    const es = new EventSource(`/api/v1/events?token=${token}`);

    es.addEventListener("data_changed", (e) => {
      const data = JSON.parse(e.data) as { source: string };
      const keys = SOURCE_QUERY_KEYS[data.source] ?? [data.source];
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
