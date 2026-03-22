// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface HealthRecord {
  id: string;
  user_id: string;
  source: string;
  record_type: string;
  value: number;
  unit: string;
  start_time: string;
  end_time?: string;
  created_at: string;
}

export interface CreateHealthRecord {
  source: string;
  record_type: string;
  value: number;
  unit: string;
  start_time: string;
}

export const healthRecordsApi = {
  list: (params?: Record<string, string>) => {
    const qs = params ? `?${new URLSearchParams(params).toString()}` : "";
    return api.get<HealthRecord[]>(`/api/v1/health-records${qs}`);
  },
  get: (id: string) => api.get<HealthRecord>(`/api/v1/health-records/${id}`),
  create: (data: CreateHealthRecord) => api.post<HealthRecord>("/api/v1/health-records", data),
  delete: (id: string) => api.delete<void>(`/api/v1/health-records/${id}`),
};
