// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface MetricSpec {
  source: string;
  field: string;
}

export interface BeforeAfterRequest {
  intervention_substance: string;
  metric: MetricSpec;
  before_days: number;
  after_days: number;
  resolution: "daily" | "weekly" | "monthly";
}

export interface WindowStats {
  mean: number;
  std_dev: number;
  n: number;
  points: Array<{ t: string; v: number }>;
}

export interface BeforeAfterResponse {
  intervention_substance: string;
  first_dose: string;
  last_dose: string | null;
  metric: MetricSpec;
  before: WindowStats;
  after: WindowStats;
  change_pct: number;
  p_value: number | null;
  significant: boolean;
  test_used: string;
}

export interface CorrelateRequest {
  metric_a: MetricSpec;
  metric_b: MetricSpec;
  start: string;
  end: string;
  resolution: "daily" | "weekly" | "monthly";
  method: "pearson" | "spearman";
}

export interface CorrelateResponse {
  metric_a: MetricSpec;
  metric_b: MetricSpec;
  r: number;
  p_value: number;
  n: number;
  significant: boolean;
  method: string;
  interpretation: string;
  scatter: Array<{ a: number; b: number; t: string }>;
}

export interface LagCorrelateRequest {
  metric_a: MetricSpec;
  metric_b: MetricSpec;
  start: string;
  end: string;
  resolution: "daily" | "weekly" | "monthly";
  max_lag_days: number;
  method: "pearson" | "spearman";
}

export interface LagEntry {
  lag: number;
  r: number;
  p_value: number;
  n: number;
}

export interface LagCorrelateResponse {
  metric_a: MetricSpec;
  metric_b: MetricSpec;
  lags: LagEntry[];
  best_lag: LagEntry;
  method: string;
}

export const statsApi = {
  beforeAfter: (data: BeforeAfterRequest) =>
    api.post<BeforeAfterResponse>("/api/v1/stats/before-after", data),

  correlate: (data: CorrelateRequest) =>
    api.post<CorrelateResponse>("/api/v1/stats/correlate", data),

  lagCorrelate: (data: LagCorrelateRequest) =>
    api.post<LagCorrelateResponse>("/api/v1/stats/lag-correlate", data),
};
