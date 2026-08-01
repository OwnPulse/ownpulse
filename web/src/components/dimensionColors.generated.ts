// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Source: docs/design/tokens.json. Regenerate with `npm run build:tokens` in tools/design-tokens.

/** The five check-in subjective-score dimensions. */
export type DimensionName = "energy" | "mood" | "focus" | "recovery" | "libido";

/**
 * Check-in dimension colors, keyed lowercase by dimension name. Single source
 * of truth for CheckinForm, ScoreRing, and SparklineRow so the five dimension
 * colors cannot drift between components.
 */
export const DIMENSION_COLORS: Record<DimensionName, string> = {
  energy: "#a78333",
  mood: "#b2573c",
  focus: "#377c7c",
  recovery: "#5a8a5a",
  libido: "#7b61c2",
};
