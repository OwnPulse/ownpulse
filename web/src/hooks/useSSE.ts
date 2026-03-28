// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { useAuthStore } from "../store/auth";

export function useSSE() {
  const token = useAuthStore((s) => s.token);
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!token) return;

    const es = new EventSource(`/api/v1/events?token=${token}`);

    es.addEventListener("data_changed", (e) => {
      const data = JSON.parse(e.data) as { source: string };
      queryClient.invalidateQueries({ queryKey: [data.source] });
      queryClient.invalidateQueries({ queryKey: ["explore-series"] });
      queryClient.invalidateQueries({ queryKey: ["dashboard-summary"] });
    });

    es.onerror = () => {
      // EventSource auto-reconnects; no action needed
    };

    return () => es.close();
  }, [token, queryClient]);
}
