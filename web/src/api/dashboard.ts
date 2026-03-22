// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface LatestCheckin {
  date: string;
  energy: number | null;
  mood: number | null;
  focus: number | null;
  recovery: number | null;
  libido: number | null;
}

export interface DashboardSummary {
  latest_checkin: LatestCheckin | null;
  checkin_count_7d: number;
  health_record_count_7d: number;
  intervention_count_7d: number;
  observation_count_7d: number;
  latest_lab_date: string | null;
  pending_friend_shares: number;
}

export const dashboardApi = {
  summary: () => api.get<DashboardSummary>("/api/v1/dashboard/summary"),
};
