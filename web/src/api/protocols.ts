// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface Protocol {
  id: string;
  user_id: string;
  name: string;
  description?: string;
  start_date: string;
  duration_days: number;
  status: string;
  share_token?: string;
  lines: ProtocolLine[];
  created_at: string;
}

export interface ProtocolLine {
  id: string;
  protocol_id: string;
  substance: string;
  dose?: number;
  unit?: string;
  route?: string;
  time_of_day?: string;
  schedule_pattern: boolean[];
  sort_order: number;
  doses: ProtocolDose[];
}

export interface ProtocolDose {
  id: string;
  protocol_line_id: string;
  day_number: number;
  status: string;
  intervention_id?: string;
  logged_at: string;
}

export interface ProtocolListItem {
  id: string;
  name: string;
  status: string;
  start_date: string;
  duration_days: number;
  progress_pct: number;
  next_dose?: string;
  created_at: string;
}

export interface TodaysDose {
  protocol_id: string;
  protocol_name: string;
  line_id: string;
  substance: string;
  dose?: number;
  unit?: string;
  route?: string;
  time_of_day?: string;
  day_number: number;
  status?: string;
}

export interface CreateProtocolLine {
  substance: string;
  dose?: number;
  unit?: string;
  route?: string;
  time_of_day?: string;
  schedule_pattern: boolean[];
  sort_order: number;
}

export interface CreateProtocol {
  name: string;
  description?: string;
  start_date: string;
  duration_days: number;
  lines: CreateProtocolLine[];
}

export interface UpdateProtocol {
  name?: string;
  description?: string;
  start_date?: string;
  duration_days?: number;
  status?: string;
}

export interface LogDoseRequest {
  line_id: string;
  day_number: number;
}

export interface SkipDoseRequest {
  line_id: string;
  day_number: number;
}

export const protocolsApi = {
  list: () => api.get<ProtocolListItem[]>("/api/v1/protocols"),
  get: (id: string) => api.get<Protocol>(`/api/v1/protocols/${id}`),
  create: (data: CreateProtocol) => api.post<Protocol>("/api/v1/protocols", data),
  update: (id: string, data: UpdateProtocol) =>
    api.patch<void>(`/api/v1/protocols/${id}`, data),
  delete: (id: string) => api.delete<void>(`/api/v1/protocols/${id}`),
  logDose: (id: string, data: LogDoseRequest) =>
    api.post<ProtocolDose>(`/api/v1/protocols/${id}/log`, data),
  skipDose: (id: string, data: SkipDoseRequest) =>
    api.post<void>(`/api/v1/protocols/${id}/skip`, data),
  share: (id: string) =>
    api.post<{ share_token: string; share_url: string }>(`/api/v1/protocols/${id}/share`, {}),
  getShared: (token: string) => api.get<Protocol>(`/api/v1/protocols/shared/${token}`),
  importProtocol: (token: string) =>
    api.post<Protocol>(`/api/v1/protocols/import/${token}`, {}),
};
