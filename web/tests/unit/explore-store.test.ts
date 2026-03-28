// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { beforeEach, describe, expect, it } from "vitest";
import { dateRangeToParams, metricKey, useExploreStore } from "../../src/stores/exploreStore";

describe("metricKey", () => {
  it("returns source:field format", () => {
    expect(metricKey({ source: "checkins", field: "energy" })).toBe("checkins:energy");
  });
});

describe("dateRangeToParams", () => {
  it("returns correct range for 7d preset", () => {
    const { start, end } = dateRangeToParams({ type: "preset", preset: "7d" });
    const startDate = new Date(start);
    const endDate = new Date(end);
    const diff = (endDate.getTime() - startDate.getTime()) / (1000 * 60 * 60 * 24);
    expect(diff).toBe(7);
  });

  it("returns correct range for 30d preset", () => {
    const { start, end } = dateRangeToParams({ type: "preset", preset: "30d" });
    const startDate = new Date(start);
    const endDate = new Date(end);
    const diff = (endDate.getTime() - startDate.getTime()) / (1000 * 60 * 60 * 24);
    expect(diff).toBe(30);
  });

  it("returns correct range for 90d preset", () => {
    const { start, end } = dateRangeToParams({ type: "preset", preset: "90d" });
    const startDate = new Date(start);
    const endDate = new Date(end);
    const diff = (endDate.getTime() - startDate.getTime()) / (1000 * 60 * 60 * 24);
    expect(diff).toBe(90);
  });

  it("returns correct range for 1y preset", () => {
    const { start, end } = dateRangeToParams({ type: "preset", preset: "1y" });
    const startDate = new Date(start);
    const endDate = new Date(end);
    const diff = (endDate.getTime() - startDate.getTime()) / (1000 * 60 * 60 * 24);
    expect(diff).toBeGreaterThanOrEqual(365);
    expect(diff).toBeLessThanOrEqual(366);
  });

  it("returns 2020-01-01 to today for all preset", () => {
    const { start, end } = dateRangeToParams({ type: "preset", preset: "all" });
    expect(start).toBe("2020-01-01");
    expect(end).toBe(new Date().toISOString().slice(0, 10));
  });

  it("returns custom dates for custom range", () => {
    const { start, end } = dateRangeToParams({
      type: "custom",
      start: "2025-01-01",
      end: "2025-06-01",
    });
    expect(start).toBe("2025-01-01");
    expect(end).toBe("2025-06-01");
  });
});

describe("useExploreStore", () => {
  beforeEach(() => {
    useExploreStore.setState({
      selectedMetrics: [],
      hiddenMetrics: new Set(),
      dateRange: { type: "preset", preset: "30d" },
      resolution: "daily",
    });
  });

  it("has correct default state", () => {
    const state = useExploreStore.getState();
    expect(state.selectedMetrics).toEqual([]);
    expect(state.hiddenMetrics.size).toBe(0);
    expect(state.dateRange).toEqual({ type: "preset", preset: "30d" });
    expect(state.resolution).toBe("daily");
  });

  describe("addMetric", () => {
    it("adds a metric to selectedMetrics", () => {
      useExploreStore.getState().addMetric({ source: "checkins", field: "energy" });
      expect(useExploreStore.getState().selectedMetrics).toEqual([
        { source: "checkins", field: "energy" },
      ]);
    });

    it("does not add duplicate metrics", () => {
      useExploreStore.getState().addMetric({ source: "checkins", field: "energy" });
      useExploreStore.getState().addMetric({ source: "checkins", field: "energy" });
      expect(useExploreStore.getState().selectedMetrics).toHaveLength(1);
    });

    it("adds multiple different metrics", () => {
      useExploreStore.getState().addMetric({ source: "checkins", field: "energy" });
      useExploreStore.getState().addMetric({ source: "checkins", field: "mood" });
      expect(useExploreStore.getState().selectedMetrics).toHaveLength(2);
    });
  });

  describe("removeMetric", () => {
    it("removes a metric from selectedMetrics", () => {
      useExploreStore.getState().addMetric({ source: "checkins", field: "energy" });
      useExploreStore.getState().addMetric({ source: "checkins", field: "mood" });
      useExploreStore.getState().removeMetric({ source: "checkins", field: "energy" });
      expect(useExploreStore.getState().selectedMetrics).toEqual([
        { source: "checkins", field: "mood" },
      ]);
    });

    it("also removes from hiddenMetrics if hidden", () => {
      useExploreStore.getState().addMetric({ source: "checkins", field: "energy" });
      useExploreStore.getState().toggleVisibility("checkins:energy");
      expect(useExploreStore.getState().hiddenMetrics.has("checkins:energy")).toBe(true);
      useExploreStore.getState().removeMetric({ source: "checkins", field: "energy" });
      expect(useExploreStore.getState().hiddenMetrics.has("checkins:energy")).toBe(false);
    });
  });

  describe("toggleVisibility", () => {
    it("hides a visible metric", () => {
      useExploreStore.getState().toggleVisibility("checkins:energy");
      expect(useExploreStore.getState().hiddenMetrics.has("checkins:energy")).toBe(true);
    });

    it("shows a hidden metric", () => {
      useExploreStore.getState().toggleVisibility("checkins:energy");
      useExploreStore.getState().toggleVisibility("checkins:energy");
      expect(useExploreStore.getState().hiddenMetrics.has("checkins:energy")).toBe(false);
    });
  });

  describe("setDateRange", () => {
    it("updates date range and auto-selects resolution", () => {
      useExploreStore.getState().setDateRange({ type: "preset", preset: "90d" });
      expect(useExploreStore.getState().dateRange).toEqual({ type: "preset", preset: "90d" });
      expect(useExploreStore.getState().resolution).toBe("weekly");
    });

    it("sets daily for 7d preset", () => {
      useExploreStore.getState().setDateRange({ type: "preset", preset: "7d" });
      expect(useExploreStore.getState().resolution).toBe("daily");
    });

    it("sets monthly for all preset", () => {
      useExploreStore.getState().setDateRange({ type: "preset", preset: "all" });
      expect(useExploreStore.getState().resolution).toBe("monthly");
    });

    it("sets daily for custom range", () => {
      useExploreStore.getState().setDateRange({
        type: "custom",
        start: "2025-01-01",
        end: "2025-03-01",
      });
      expect(useExploreStore.getState().resolution).toBe("daily");
    });
  });

  describe("setResolution", () => {
    it("overrides auto-selected resolution", () => {
      useExploreStore.getState().setDateRange({ type: "preset", preset: "all" });
      expect(useExploreStore.getState().resolution).toBe("monthly");
      useExploreStore.getState().setResolution("daily");
      expect(useExploreStore.getState().resolution).toBe("daily");
    });
  });

  describe("clearAll", () => {
    it("resets all state to defaults", () => {
      useExploreStore.getState().addMetric({ source: "checkins", field: "energy" });
      useExploreStore.getState().setDateRange({ type: "preset", preset: "90d" });
      useExploreStore.getState().toggleVisibility("checkins:energy");
      useExploreStore.getState().clearAll();

      const state = useExploreStore.getState();
      expect(state.selectedMetrics).toEqual([]);
      expect(state.hiddenMetrics.size).toBe(0);
      expect(state.dateRange).toEqual({ type: "preset", preset: "30d" });
      expect(state.resolution).toBe("daily");
    });
  });

  describe("loadConfig", () => {
    it("loads a saved chart config with preset range", () => {
      useExploreStore.getState().loadConfig({
        metrics: [
          { source: "checkins", field: "energy" },
          { source: "checkins", field: "mood" },
        ],
        range: { preset: "7d" },
        resolution: "daily",
      });

      const state = useExploreStore.getState();
      expect(state.selectedMetrics).toHaveLength(2);
      expect(state.dateRange).toEqual({ type: "preset", preset: "7d" });
      expect(state.resolution).toBe("daily");
      expect(state.hiddenMetrics.size).toBe(0);
    });

    it("loads a saved chart config with custom range", () => {
      useExploreStore.getState().loadConfig({
        metrics: [{ source: "health_records", field: "weight" }],
        range: { start: "2025-01-01", end: "2025-06-01" },
        resolution: "weekly",
      });

      const state = useExploreStore.getState();
      expect(state.selectedMetrics).toHaveLength(1);
      expect(state.dateRange).toEqual({
        type: "custom",
        start: "2025-01-01",
        end: "2025-06-01",
      });
      expect(state.resolution).toBe("weekly");
    });

    it("defaults to 30d preset if range is malformed", () => {
      useExploreStore.getState().loadConfig({
        metrics: [],
        range: {},
        resolution: "daily",
      });
      expect(useExploreStore.getState().dateRange).toEqual({ type: "preset", preset: "30d" });
    });

    it("defaults to daily if resolution is invalid", () => {
      useExploreStore.getState().loadConfig({
        metrics: [],
        range: { preset: "30d" },
        resolution: "invalid",
      });
      expect(useExploreStore.getState().resolution).toBe("daily");
    });
  });
});
