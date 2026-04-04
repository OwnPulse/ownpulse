// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { create } from "zustand";

export interface MetricRef {
  source: string;
  field: string;
}

export type DateRange =
  | { type: "preset"; preset: "7d" | "30d" | "90d" | "1y" | "all" }
  | { type: "custom"; start: string; end: string };

export type Resolution = "daily" | "weekly" | "monthly";

interface ExploreState {
  selectedMetrics: MetricRef[];
  hiddenMetrics: Set<string>;
  dateRange: DateRange;
  resolution: Resolution;
  zoomRange: [number, number] | null;
  hiddenSubstances: string[];

  addMetric: (m: MetricRef) => void;
  removeMetric: (m: MetricRef) => void;
  toggleVisibility: (key: string) => void;
  setDateRange: (r: DateRange) => void;
  setResolution: (r: Resolution) => void;
  setZoomRange: (range: [number, number] | null) => void;
  resetZoom: () => void;
  toggleSubstanceVisibility: (substance: string) => void;
  clearAll: () => void;
  loadConfig: (config: {
    metrics: Array<{ source: string; field: string }>;
    range: { preset?: string; start?: string; end?: string };
    resolution: string;
  }) => void;
}

export function metricKey(m: MetricRef): string {
  return `${m.source}:${m.field}`;
}

function defaultResolutionForPreset(preset: string): Resolution {
  switch (preset) {
    case "7d":
    case "30d":
      return "daily";
    case "90d":
    case "1y":
      return "weekly";
    case "all":
      return "monthly";
    default:
      return "daily";
  }
}

export function dateRangeToParams(range: DateRange): { start: string; end: string } {
  if (range.type === "custom") {
    return { start: `${range.start}T00:00:00Z`, end: `${range.end}T23:59:59Z` };
  }

  const now = new Date();
  const endDate = now.toISOString().slice(0, 10);

  let startDate: string;
  switch (range.preset) {
    case "7d": {
      const d = new Date(now);
      d.setDate(d.getDate() - 7);
      startDate = d.toISOString().slice(0, 10);
      break;
    }
    case "30d": {
      const d = new Date(now);
      d.setDate(d.getDate() - 30);
      startDate = d.toISOString().slice(0, 10);
      break;
    }
    case "90d": {
      const d = new Date(now);
      d.setDate(d.getDate() - 90);
      startDate = d.toISOString().slice(0, 10);
      break;
    }
    case "1y": {
      const d = new Date(now);
      d.setFullYear(d.getFullYear() - 1);
      startDate = d.toISOString().slice(0, 10);
      break;
    }
    default:
      startDate = "2020-01-01";
      break;
  }

  return { start: `${startDate}T00:00:00Z`, end: `${endDate}T23:59:59Z` };
}

export const useExploreStore = create<ExploreState>((set) => ({
  selectedMetrics: [],
  hiddenMetrics: new Set<string>(),
  dateRange: { type: "preset", preset: "30d" },
  resolution: "daily",
  zoomRange: null,
  hiddenSubstances: [],

  addMetric: (m) =>
    set((state) => {
      const key = metricKey(m);
      if (state.selectedMetrics.some((sm) => metricKey(sm) === key)) {
        return state;
      }
      return { selectedMetrics: [...state.selectedMetrics, m] };
    }),

  removeMetric: (m) =>
    set((state) => {
      const key = metricKey(m);
      const newHidden = new Set(state.hiddenMetrics);
      newHidden.delete(key);
      return {
        selectedMetrics: state.selectedMetrics.filter((sm) => metricKey(sm) !== key),
        hiddenMetrics: newHidden,
      };
    }),

  toggleVisibility: (key) =>
    set((state) => {
      const newHidden = new Set(state.hiddenMetrics);
      if (newHidden.has(key)) {
        newHidden.delete(key);
      } else {
        newHidden.add(key);
      }
      return { hiddenMetrics: newHidden };
    }),

  setDateRange: (r) =>
    set(() => {
      const resolution = r.type === "preset" ? defaultResolutionForPreset(r.preset) : "daily";
      return { dateRange: r, resolution, zoomRange: null };
    }),

  setResolution: (r) => set({ resolution: r, zoomRange: null }),

  setZoomRange: (range) => set({ zoomRange: range }),

  resetZoom: () => set({ zoomRange: null }),

  toggleSubstanceVisibility: (substance) =>
    set((state) => {
      const idx = state.hiddenSubstances.indexOf(substance);
      if (idx >= 0) {
        return { hiddenSubstances: state.hiddenSubstances.filter((s) => s !== substance) };
      }
      return { hiddenSubstances: [...state.hiddenSubstances, substance] };
    }),

  clearAll: () =>
    set({
      selectedMetrics: [],
      hiddenMetrics: new Set<string>(),
      dateRange: { type: "preset", preset: "30d" },
      resolution: "daily",
      zoomRange: null,
      hiddenSubstances: [],
    }),

  loadConfig: (config) =>
    set(() => {
      const metrics = config.metrics.map((m) => ({ source: m.source, field: m.field }));
      let dateRange: DateRange;
      if (config.range.preset) {
        dateRange = {
          type: "preset",
          preset: config.range.preset as "7d" | "30d" | "90d" | "1y" | "all",
        };
      } else if (config.range.start && config.range.end) {
        dateRange = { type: "custom", start: config.range.start, end: config.range.end };
      } else {
        dateRange = { type: "preset", preset: "30d" };
      }
      const resolution = (
        ["daily", "weekly", "monthly"].includes(config.resolution) ? config.resolution : "daily"
      ) as Resolution;
      return {
        selectedMetrics: metrics,
        hiddenMetrics: new Set<string>(),
        dateRange,
        resolution,
      };
    }),
}));
