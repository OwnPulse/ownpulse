// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Source: docs/design/tokens.json. Regenerate with `npm run build:tokens` in tools/design-tokens.

/** Per-metric chart colors, keyed by canonical metric name. */
export const METRIC_COLORS: Record<string, string> = {
  bp_diastolic: "#56b4e9",
  bp_systolic: "#cc79a7",
  glucose: "#0072b2",
  heart_rate: "#d55e00",
  hrv: "#009e73",
  sleep_duration: "#7b61c2",
  weight: "#c49a3c",
};

/** Deterministic fallback cycle for metrics without a dedicated color. */
export const FALLBACK_COLORS: readonly string[] = [
  "#c2654a",
  "#e69f00",
  "#56b4e9",
  "#009e73",
  "#d4a017",
  "#0072b2",
  "#d55e00",
  "#cc79a7",
  "#5b8a72",
  "#88ccee",
  "#44aa99",
  "#ddcc77",
];

/** Backend `record_type` field names that are synonyms for a canonical metric key. */
export const METRIC_ALIASES: Record<string, string> = {
  blood_glucose: "glucose",
  blood_pressure_diastolic: "bp_diastolic",
  blood_pressure_systolic: "bp_systolic",
  body_mass: "weight",
  heart_rate_variability: "hrv",
  resting_heart_rate: "heart_rate",
  sleep_analysis: "sleep_duration",
};
