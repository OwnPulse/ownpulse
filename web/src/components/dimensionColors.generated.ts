// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Source: docs/design/tokens.json. Regenerate with `npm run build:tokens` in tools/design-tokens.

/**
 * Check-in dimension colors, keyed lowercase by dimension name. Single source
 * of truth for CheckinForm, ScoreRing, and SparklineRow so the five dimension
 * colors cannot drift between components.
 */
export const DIMENSION_COLORS: Record<string, string> = {
  energy: "#c49a3c",
  mood: "#b2573c",
  focus: "#377c7c",
  recovery: "#5a8a5a",
  libido: "#7b61c2",
};

export const ENERGY_COLOR: string = "#c49a3c";
export const MOOD_COLOR: string = "#b2573c";
export const FOCUS_COLOR: string = "#377c7c";
export const RECOVERY_COLOR: string = "#5a8a5a";
export const LIBIDO_COLOR: string = "#7b61c2";
