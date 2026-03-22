// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface Observation {
  id: string;
  user_id: string;
  type: string;
  name: string;
  start_time: string;
  end_time?: string;
  value: Record<string, unknown>;
  created_at: string;
}

export interface CreateObservation {
  type: string;
  name: string;
  start_time: string;
  end_time?: string;
  value: Record<string, unknown>;
}

export const observationsApi = {
  list: (params?: Record<string, string>) => {
    const qs = params ? `?${new URLSearchParams(params).toString()}` : "";
    return api.get<Observation[]>(`/api/v1/observations${qs}`);
  },
  get: (id: string) => api.get<Observation>(`/api/v1/observations/${id}`),
  create: (data: CreateObservation) => api.post<Observation>("/api/v1/observations", data),
  delete: (id: string) => api.delete<void>(`/api/v1/observations/${id}`),
};
