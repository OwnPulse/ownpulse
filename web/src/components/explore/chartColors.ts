// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

import { FALLBACK_COLORS, METRIC_ALIASES, METRIC_COLORS } from "./chartMetricColors.generated";

// Per-metric chart colors are token-driven (single source of truth in
// docs/design/tokens.json + the alias table in tools/design-tokens/build.js).
// A known metric always renders in its dedicated token color; unknown metrics
// fall back to a deterministic cycle. The alias map (e.g. body_mass -> weight)
// is generated from the same source as iOS, so the two platforms cannot drift.
// Never hardcode hex here — extend the tokens instead.

/** Fallback color cycle for metrics without a dedicated token color. */
export const CHART_COLORS: readonly string[] = FALLBACK_COLORS;

/**
 * Resolve a metric's chart color from the token-derived keyed map.
 *
 * Returns the dedicated token color when the field (or one of its aliases) has
 * one; otherwise returns the deterministic fallback-cycle color for `index`.
 */
export function colorForMetric(field: string, index: number): string {
  const key = METRIC_ALIASES[field] ?? field;
  const mapped = METRIC_COLORS[key];
  if (mapped) return mapped;
  const cycle = FALLBACK_COLORS;
  const i = ((index % cycle.length) + cycle.length) % cycle.length;
  return cycle[i];
}

export const LINE_STYLES: Array<"solid" | "dashed" | number[]> = [
  "solid",
  "dashed",
  [4, 4],
  [8, 4, 2, 4],
];

export const INTERVENTION_COLOR = "#9b59b6";
