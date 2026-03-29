// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface Insight {
  id: string;
  insight_type: "trend" | "anomaly" | "missing_data" | "streak" | "correlation";
  headline: string;
  detail: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
}

export const insightsApi = {
  list: () => api.get<Insight[]>("/api/v1/insights"),
  dismiss: (id: string) => api.post<void>(`/api/v1/insights/${id}/dismiss`, {}),
  generate: () => api.post<Insight[]>("/api/v1/insights/generate", {}),
};
