// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface ProtocolDose {
  id: string;
  protocol_line_id: string;
  day_number: number;
  status: "completed" | "skipped" | "pending";
  intervention_id: string | null;
  logged_at: string | null;
  created_at: string;
}

export interface ProtocolLine {
  id: string;
  protocol_id: string;
  substance: string;
  dose: number;
  unit: string;
  route: string;
  time_of_day: string | null;
  schedule_pattern: boolean[];
  sort_order: number;
  doses: ProtocolDose[];
}

export interface Protocol {
  id: string;
  user_id: string;
  name: string;
  description: string | null;
  status: "active" | "paused" | "completed";
  start_date: string;
  duration_days: number;
  share_token: string | null;
  created_at: string;
  updated_at: string;
  lines: ProtocolLine[];
}

export interface ProtocolListItem {
  id: string;
  name: string;
  status: "active" | "paused" | "completed";
  start_date: string;
  duration_days: number;
  created_at: string;
  lines: ProtocolLine[];
}

export interface TodaysDose {
  protocol_id: string;
  protocol_name: string;
  protocol_line_id: string;
  substance: string;
  dose: number;
  unit: string;
  route: string;
  time_of_day: string | null;
  day_number: number;
  status: "completed" | "skipped" | "pending";
  dose_id: string | null;
}

export interface LogDoseRequest {
  protocol_line_id: string;
  day_number: number;
}

export interface SkipDoseRequest {
  protocol_line_id: string;
  day_number: number;
}

export interface ShareResponse {
  share_token: string;
  share_url: string;
}

export const protocolsApi = {
  list: (params?: Record<string, string>) => {
    const qs = params ? `?${new URLSearchParams(params).toString()}` : "";
    return api.get<ProtocolListItem[]>(`/api/v1/protocols${qs}`);
  },
  get: (id: string) => api.get<Protocol>(`/api/v1/protocols/${id}`),
  update: (id: string, data: Partial<Pick<Protocol, "name" | "description" | "status">>) =>
    api.patch<Protocol>(`/api/v1/protocols/${id}`, data),
  delete: (id: string) => api.delete<void>(`/api/v1/protocols/${id}`),
  logDose: (protocolId: string, data: LogDoseRequest) =>
    api.post<ProtocolDose>(`/api/v1/protocols/${protocolId}/doses/log`, data),
  skipDose: (protocolId: string, data: SkipDoseRequest) =>
    api.post<ProtocolDose>(`/api/v1/protocols/${protocolId}/doses/skip`, data),
  share: (id: string) => api.post<ShareResponse>(`/api/v1/protocols/${id}/share`, {}),
  getShared: (token: string) => api.get<Protocol>(`/api/v1/protocols/shared/${token}`),
  importProtocol: (token: string) =>
    api.post<Protocol>("/api/v1/protocols/import", { share_token: token }),
};
