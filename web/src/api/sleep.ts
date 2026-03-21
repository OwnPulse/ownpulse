// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface SleepRecord {
  id: string;
  user_id: string;
  date: string;
  sleep_start: string | null;
  sleep_end: string | null;
  duration_minutes: number;
  deep_minutes: number | null;
  light_minutes: number | null;
  rem_minutes: number | null;
  awake_minutes: number | null;
  score: number | null;
  source: string;
  source_id: string | null;
  notes: string | null;
  created_at: string;
}

export interface CreateSleep {
  date: string;
  sleep_start?: string;
  sleep_end?: string;
  duration_minutes: number;
  deep_minutes?: number;
  light_minutes?: number;
  rem_minutes?: number;
  awake_minutes?: number;
  score?: number;
  source: string;
  source_id?: string;
  notes?: string;
}

export const sleepApi = {
  list: (params?: Record<string, string>) => {
    const qs = params ? `?${new URLSearchParams(params).toString()}` : "";
    return api.get<SleepRecord[]>(`/api/v1/sleep${qs}`);
  },
  get: (id: string) => api.get<SleepRecord>(`/api/v1/sleep/${id}`),
  create: (data: CreateSleep) => api.post<SleepRecord>("/api/v1/sleep", data),
  delete: (id: string) => api.delete<void>(`/api/v1/sleep/${id}`),
};
