// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

// Unit tests for the keyed per-metric chart-color lookup (chartColors.ts).
// Locks in three properties:
//   (a) a known metric resolves to its dedicated token color, regardless of index
//   (b) field-name aliases resolve to the canonical metric color
//   (c) an unknown metric resolves to the deterministic fallback cycle
// The expected colors are read from the generated token map, never hardcoded
// hex — that keeps the test honest if the token source changes.

import { describe, expect, it } from "vitest";

import { CHART_COLORS, colorForMetric } from "../../src/components/explore/chartColors";
import {
  FALLBACK_COLORS,
  METRIC_ALIASES,
  METRIC_COLORS,
} from "../../src/components/explore/chartMetricColors.generated";

// The ACTUAL backend `record_type` field strings the explore API emits for each
// token-keyed metric (backend/api/src/models/explore.rs). These — not the token
// keys — are what `colorForMetric` receives in production, so the tests must use
// them. If the backend renames a field, these must move in lockstep.
const FIELD_TO_TOKEN_KEY: Record<string, string> = {
  heart_rate: "heart_rate",
  resting_heart_rate: "heart_rate",
  heart_rate_variability: "hrv",
  blood_pressure_systolic: "bp_systolic",
  blood_pressure_diastolic: "bp_diastolic",
  blood_glucose: "glucose",
  body_mass: "weight",
  sleep_analysis: "sleep_duration",
};

describe("colorForMetric — real backend field names resolve to their token color", () => {
  it.each(
    Object.entries(FIELD_TO_TOKEN_KEY),
  )("field %s resolves to the %s token color regardless of index", (field, tokenKey) => {
    const expected = METRIC_COLORS[tokenKey];
    expect(expected).toBeDefined();
    expect(colorForMetric(field, 0)).toBe(expected);
    expect(colorForMetric(field, 5)).toBe(expected);
    expect(colorForMetric(field, 99)).toBe(expected);
  });

  it("gives glucose and the two blood-pressure fields distinct, dedicated colors", () => {
    // Regression guard: these three previously fell through to the fallback
    // cycle because their alias entries used the wrong (token-key) field names.
    expect(colorForMetric("blood_glucose", 0)).toBe(METRIC_COLORS.glucose);
    expect(colorForMetric("blood_pressure_systolic", 0)).toBe(METRIC_COLORS.bp_systolic);
    expect(colorForMetric("blood_pressure_diastolic", 0)).toBe(METRIC_COLORS.bp_diastolic);
    expect(colorForMetric("blood_glucose", 0)).not.toBe(FALLBACK_COLORS[0]);
    expect(colorForMetric("blood_pressure_systolic", 0)).not.toBe(FALLBACK_COLORS[0]);
  });
});

describe("colorForMetric — canonical token keys also resolve (no alias needed)", () => {
  it.each(
    Object.keys(METRIC_COLORS),
  )("%s always resolves to its token color regardless of index", (metric) => {
    const expected = METRIC_COLORS[metric];
    expect(colorForMetric(metric, 0)).toBe(expected);
    expect(colorForMetric(metric, 99)).toBe(expected);
  });
});

describe("every metric color is reachable from a real backend field", () => {
  it("each token key is the target of an alias or equals a real field name", () => {
    const reachable = new Set<string>();
    for (const tokenKey of Object.values(FIELD_TO_TOKEN_KEY)) reachable.add(tokenKey);
    for (const tokenKey of Object.values(METRIC_ALIASES)) reachable.add(tokenKey);
    for (const key of Object.keys(METRIC_COLORS)) {
      expect(reachable.has(key)).toBe(true);
    }
  });

  it("every generated alias is exercised by a known backend field", () => {
    // Guards against a stale alias pointing at a field the backend never emits.
    for (const field of Object.keys(METRIC_ALIASES)) {
      expect(FIELD_TO_TOKEN_KEY[field]).toBeDefined();
    }
  });
});

describe("colorForMetric — unknown metrics fall back deterministically", () => {
  it("indexes the fallback cycle by position", () => {
    expect(colorForMetric("unknown_metric", 0)).toBe(FALLBACK_COLORS[0]);
    expect(colorForMetric("unknown_metric", 1)).toBe(FALLBACK_COLORS[1]);
  });

  it("wraps around the fallback cycle", () => {
    const n = FALLBACK_COLORS.length;
    expect(colorForMetric("unknown_metric", n)).toBe(FALLBACK_COLORS[0]);
    expect(colorForMetric("unknown_metric", n + 2)).toBe(FALLBACK_COLORS[2]);
  });

  it("is stable for the same metric + index", () => {
    expect(colorForMetric("steps", 4)).toBe(colorForMetric("steps", 4));
  });
});

describe("CHART_COLORS export surface", () => {
  it("remains the fallback cycle for legacy callers", () => {
    expect(CHART_COLORS).toEqual(FALLBACK_COLORS);
  });
});
