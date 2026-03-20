// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface Checkin {
  id: string;
  user_id: string;
  date: string;
  energy: number;
  mood: number;
  focus: number;
  recovery: number;
  libido: number;
  notes?: string;
  created_at: string;
}

export interface UpsertCheckin {
  date: string;
  energy: number;
  mood: number;
  focus: number;
  recovery: number;
  libido: number;
  notes?: string;
}

export const checkinsApi = {
  list: (params?: Record<string, string>) => {
    const qs = params ? `?${new URLSearchParams(params).toString()}` : "";
    return api.get<Checkin[]>(`/api/v1/checkins${qs}`);
  },
  get: (id: string) => api.get<Checkin>(`/api/v1/checkins/${id}`),
  upsert: (data: UpsertCheckin) =>
    api.put<Checkin>("/api/v1/checkins", data),
  delete: (id: string) => api.delete<void>(`/api/v1/checkins/${id}`),
};
