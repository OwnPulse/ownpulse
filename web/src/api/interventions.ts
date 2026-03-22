// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface Intervention {
  id: string;
  user_id: string;
  substance: string;
  dose: number;
  unit: string;
  route: string;
  administered_at: string;
  fasted: boolean;
  notes?: string;
  created_at: string;
}

export interface CreateIntervention {
  substance: string;
  dose: number;
  unit: string;
  route: string;
  administered_at: string;
  fasted: boolean;
  notes?: string;
}

export const interventionsApi = {
  list: (params?: Record<string, string>) => {
    const qs = params ? `?${new URLSearchParams(params).toString()}` : "";
    return api.get<Intervention[]>(`/api/v1/interventions${qs}`);
  },
  get: (id: string) => api.get<Intervention>(`/api/v1/interventions/${id}`),
  create: (data: CreateIntervention) => api.post<Intervention>("/api/v1/interventions", data),
  delete: (id: string) => api.delete<void>(`/api/v1/interventions/${id}`),
};
