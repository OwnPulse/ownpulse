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
  status: "active" | "paused" | "completed" | "draft" | "archived";
  start_date?: string;
  duration_days: number;
  share_token: string | null;
  created_at: string;
  updated_at: string;
  lines: ProtocolLine[];
}

export interface ProtocolListItem {
  id: string;
  name: string;
  description?: string;
  status: "active" | "paused" | "completed" | "draft" | "archived";
  start_date?: string;
  duration_days: number;
  is_template?: boolean;
  tags?: string[];
  progress_pct?: number;
  next_dose?: string;
  created_at: string;
  // lines is NOT returned in the list endpoint — only in GET by ID
}

export interface TodaysDose {
  protocol_id: string;
  protocol_name: string;
  protocol_line_id: string;
  run_id: string;
  substance: string;
  dose: number;
  unit: string;
  route: string;
  time_of_day: string | null;
  day_number: number;
  status: "completed" | "skipped" | "pending";
  dose_id: string | null;
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
  start_date?: string;
  duration_days: number;
  lines: CreateProtocolLine[];
}

export interface LogDoseRequest {
  protocol_line_id: string;
  day_number: number;
}

export interface SkipDoseRequest {
  protocol_line_id: string;
  day_number: number;
}

export interface ProtocolRun {
  id: string;
  protocol_id: string;
  user_id: string;
  start_date: string;
  status: "active" | "paused" | "completed";
  notify: boolean;
  notify_times: string[];
  repeat_reminders: boolean;
  repeat_interval_minutes: number;
  created_at: string;
}

export interface CreateRunRequest {
  start_date?: string;
  notify?: boolean;
  notify_times?: string[];
  repeat_reminders?: boolean;
  repeat_interval_minutes?: number;
}

export interface UpdateRunRequest {
  status?: "active" | "paused" | "completed";
  notify?: boolean;
  notify_times?: string[];
  repeat_reminders?: boolean;
  repeat_interval_minutes?: number;
}

export interface ActiveRunResponse {
  id: string;
  protocol_id: string;
  protocol_name: string | null;
  user_id: string;
  start_date: string;
  duration_days: number | null;
  status: "active" | "paused" | "completed";
  notify: boolean;
  notify_time: string | null;
  notify_times: string[] | null;
  repeat_reminders: boolean;
  repeat_interval_minutes: number | null;
  progress_pct: number;
  doses_today: number;
  doses_completed_today: number;
  created_at: string;
}

export interface ShareResponse {
  share_token: string;
  share_url: string;
}

export interface ProtocolLineExport {
  substance: string;
  dose?: number;
  unit?: string;
  route?: string;
  time_of_day?: string;
  pattern: string | boolean[];
}

export interface ProtocolExport {
  schema: string;
  name: string;
  description?: string;
  tags: string[];
  duration_days: number;
  lines: ProtocolLineExport[];
}

export interface TemplateListItem {
  id: string;
  name: string;
  description: string | null;
  tags: string[];
  duration_days: number;
  line_count: number;
}

export interface ActiveSubstance {
  substance: string;
  dose: number;
  unit: string;
  route: string;
  protocol_name: string;
  protocol_id: string;
}

export const protocolsApi = {
  list: (params?: Record<string, string>) => {
    const qs = params ? `?${new URLSearchParams(params).toString()}` : "";
    return api.get<ProtocolListItem[]>(`/api/v1/protocols${qs}`);
  },
  get: (id: string) => api.get<Protocol>(`/api/v1/protocols/${id}`),
  create: (data: CreateProtocol) => api.post<Protocol>("/api/v1/protocols", data),
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
  exportProtocol: (id: string) => api.get<ProtocolExport>(`/api/v1/protocols/${id}/export`),
  importFromFile: (data: ProtocolExport) =>
    api.post<Protocol>("/api/v1/protocols/import-file", data),
  activeSubstances: () => api.get<ActiveSubstance[]>("/api/v1/protocols/active-substances"),
  listTemplates: () => api.get<TemplateListItem[]>("/api/v1/protocols/templates"),
  copyTemplate: (id: string, startDate: string) =>
    api.post<Protocol>(`/api/v1/protocols/templates/${id}/copy`, { start_date: startDate }),

  // Protocol runs
  startRun: (protocolId: string, data: CreateRunRequest) =>
    api.post<ProtocolRun>(`/api/v1/protocols/${protocolId}/runs`, data),
  listRuns: (protocolId: string) => api.get<ProtocolRun[]>(`/api/v1/protocols/${protocolId}/runs`),
  activeRuns: () => api.get<ActiveRunResponse[]>("/api/v1/protocols/runs/active"),
  updateRun: (runId: string, data: UpdateRunRequest) =>
    api.patch<ProtocolRun>(`/api/v1/protocols/runs/${runId}`, data),

  // Run doses
  todaysDoses: () => api.get<TodaysDose[]>("/api/v1/protocols/runs/todays-doses"),
  logRunDose: (runId: string, data: LogDoseRequest) =>
    api.post<ProtocolDose>(`/api/v1/protocols/runs/${runId}/doses/log`, data),
  skipRunDose: (runId: string, data: SkipDoseRequest) =>
    api.post<ProtocolDose>(`/api/v1/protocols/runs/${runId}/doses/skip`, data),
};
