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
  // No `created_at` here — `ProtocolDoseRow` in the backend never serves it.
  // The run this dose belongs to; `null` for legacy protocol-level doses
  // logged before runs existed.
  run_id: string | null;
  skip_reason: string | null;
}

export interface ProtocolLine {
  id: string;
  protocol_id: string;
  substance: string;
  dose: number | null;
  unit: string | null;
  route: string | null;
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
  dose: number | null;
  unit: string | null;
  route: string | null;
  time_of_day: string | null;
  day_number: number;
  status: "completed" | "skipped" | "pending" | null;
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
  /** Must fall within a day of the scheduled date (in `tz_offset_minutes`). */
  administered_at?: string;
  notes?: string;
  /** Caller's local UTC offset in minutes, e.g. `-420` for UTC-7. */
  tz_offset_minutes?: number;
}

export interface SkipDoseRequest {
  protocol_line_id: string;
  day_number: number;
  skip_reason?: string;
}

export interface ProtocolRun {
  id: string;
  protocol_id: string;
  protocol_name: string | null;
  user_id: string;
  start_date: string;
  duration_days: number | null;
  status: "active" | "paused" | "completed";
  notify: boolean;
  notify_times: string[];
  repeat_reminders: boolean;
  repeat_interval_minutes: number;
  progress_pct: number;
  doses_today: number;
  doses_completed_today: number;
  /** `null` when the denominator is 0 (nothing scheduled yet, or every
   *  closed day was skipped) — see `RunResponse.adherence_pct` in the
   *  backend for the exact rule. */
  adherence_pct: number | null;
  doses_missed: number | null;
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
  adherence_pct: number | null;
  doses_missed: number | null;
  created_at: string;
}

export interface ShareResponse {
  token: string;
  expires_at: string;
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
  dose: number | null;
  unit: string | null;
  route: string | null;
  protocol_name: string;
  protocol_id: string;
}

/** One entry of `GET /protocols/runs/:run_id/doses` — a scheduled (line,
 *  day) pair with its server-computed status. */
export interface RunDoseItem {
  day_number: number;
  date: string;
  protocol_line_id: string;
  substance: string;
  dose: number | null;
  unit: string | null;
  route: string | null;
  time_of_day: string | null;
  status: "completed" | "skipped" | "missed" | "pending";
  dose_id: string | null;
  intervention_id: string | null;
  skip_reason: string | null;
  logged_at: string | null;
}

/** One entry of `GET /protocols/runs/missed-doses` — a scheduled day, in
 *  the past, across the caller's active runs, with no dose row. Capped at
 *  200 rows server-side. */
export interface MissedDoseItem {
  protocol_id: string;
  protocol_name: string;
  run_id: string;
  protocol_line_id: string;
  substance: string;
  dose: number | null;
  unit: string | null;
  route: string | null;
  time_of_day: string | null;
  day_number: number;
  date: string;
  status: "missed";
}

export interface LineAdherence {
  protocol_line_id: string;
  substance: string;
  scheduled_so_far: number;
  completed: number;
  skipped: number;
  missed: number;
  adherence_pct: number | null;
}

/** Response of `GET /protocols/runs/:run_id/adherence` — computed over
 *  closed days only (scheduled days strictly before today, excluding
 *  paused days); skips are excluded from the adherence denominator. */
export interface AdherenceResponse {
  run_id: string;
  scheduled_so_far: number;
  completed: number;
  skipped: number;
  missed: number;
  adherence_pct: number | null;
  lines: LineAdherence[];
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
  share: (id: string) => api.post<ShareResponse>(`/api/v1/protocols/${id}/share`, {}),
  getShared: (token: string) => api.get<Protocol>(`/api/v1/protocols/shared/${token}`),
  importProtocol: (token: string) => api.post<Protocol>(`/api/v1/protocols/import/${token}`, {}),
  exportProtocol: (id: string) => api.get<ProtocolExport>(`/api/v1/protocols/${id}/export`),
  importFromFile: (data: ProtocolExport) => api.post<Protocol>("/api/v1/protocols/import", data),
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
    // Always send tz_offset_minutes — the backend uses it to resolve
    // "today"/the default dose time in the caller's own calendar day
    // rather than the database server's, so every write must carry it,
    // not just the ones a caller happens to set explicitly.
    api.post<ProtocolDose>(`/api/v1/protocols/runs/${runId}/doses/log`, {
      ...data,
      tz_offset_minutes: data.tz_offset_minutes ?? -new Date().getTimezoneOffset(),
    }),
  skipRunDose: (runId: string, data: SkipDoseRequest) =>
    api.post<ProtocolDose>(`/api/v1/protocols/runs/${runId}/doses/skip`, data),
  deleteRunDose: (runId: string, doseId: string) =>
    api.delete<void>(`/api/v1/protocols/runs/${runId}/doses/${doseId}`),
  runDoses: (runId: string, range?: { fromDay?: number; toDay?: number }) => {
    const params = new URLSearchParams();
    if (range?.fromDay !== undefined) params.set("from_day", String(range.fromDay));
    if (range?.toDay !== undefined) params.set("to_day", String(range.toDay));
    const qs = params.toString() ? `?${params.toString()}` : "";
    return api.get<RunDoseItem[]>(`/api/v1/protocols/runs/${runId}/doses${qs}`);
  },
  missedDoses: () => api.get<MissedDoseItem[]>("/api/v1/protocols/runs/missed-doses"),
  runAdherence: (runId: string) =>
    api.get<AdherenceResponse>(`/api/v1/protocols/runs/${runId}/adherence`),
};
