// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface LabResult {
  id: string;
  user_id: string;
  panel_date: string;
  lab_name: string;
  marker: string;
  value: number;
  unit: string;
  reference_low?: number;
  reference_high?: number;
  created_at: string;
}

export interface CreateLabResult {
  panel_date: string;
  lab_name: string;
  marker: string;
  value: number;
  unit: string;
  reference_low?: number;
  reference_high?: number;
}

export const labsApi = {
  list: (params?: Record<string, string>) => {
    const qs = params ? `?${new URLSearchParams(params).toString()}` : "";
    return api.get<LabResult[]>(`/api/v1/labs${qs}`);
  },
  get: (id: string) => api.get<LabResult>(`/api/v1/labs/${id}`),
  create: (data: CreateLabResult) => api.post<LabResult>("/api/v1/labs", data),
  delete: (id: string) => api.delete<void>(`/api/v1/labs/${id}`),
};
