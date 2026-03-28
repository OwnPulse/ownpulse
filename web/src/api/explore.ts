// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { api } from "./client";

export interface MetricOption {
  field: string;
  label: string;
  unit: string;
}

export interface MetricSourceGroup {
  source: string;
  label: string;
  metrics: MetricOption[];
}

export interface MetricsResponse {
  sources: MetricSourceGroup[];
}

export interface DataPoint {
  t: string;
  v: number;
  n: number;
}

export interface SeriesResponse {
  source: string;
  field: string;
  unit: string;
  points: DataPoint[];
}

export interface MetricSpec {
  source: string;
  field: string;
}

export interface BatchSeriesRequest {
  metrics: MetricSpec[];
  start: string;
  end: string;
  resolution: "daily" | "weekly" | "monthly";
}

export interface BatchSeriesResponse {
  series: SeriesResponse[];
}

export interface ChartConfig {
  version: 1;
  metrics: Array<{ source: string; field: string; color?: string }>;
  range: { preset: string } | { start: string; end: string };
  resolution: "daily" | "weekly" | "monthly";
}

export interface SavedChart {
  id: string;
  name: string;
  config: ChartConfig;
  created_at: string;
  updated_at: string;
}

export const exploreApi = {
  getMetrics: () => api.get<MetricsResponse>("/api/v1/explore/metrics"),

  getSeries: (params: {
    source: string;
    field: string;
    start: string;
    end: string;
    resolution: string;
  }) => {
    const qs = new URLSearchParams(params).toString();
    return api.get<SeriesResponse>(`/api/v1/explore/series?${qs}`);
  },

  batchSeries: (data: BatchSeriesRequest) =>
    api.post<BatchSeriesResponse>("/api/v1/explore/series", data),

  listCharts: () => api.get<SavedChart[]>("/api/v1/explore/charts"),

  createChart: (data: { name: string; config: ChartConfig }) =>
    api.post<SavedChart>("/api/v1/explore/charts", data),

  getChart: (id: string) => api.get<SavedChart>(`/api/v1/explore/charts/${id}`),

  updateChart: (id: string, data: { name?: string; config?: ChartConfig }) =>
    api.put<SavedChart>(`/api/v1/explore/charts/${id}`, data),

  deleteChart: (id: string) => api.delete<void>(`/api/v1/explore/charts/${id}`),
};
