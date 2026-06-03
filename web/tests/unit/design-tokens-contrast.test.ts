// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

// Unit tests for the WCAG 2.1 contrast checker (tools/design-tokens/contrast.js).
//
// The checker is a Node/JS tool with no React surface, so — like the
// design-tokens generator test — it is exercised here in the web Vitest suite,
// the project's only JS unit-test runner. These tests lock in:
//   (a) the relative-contrast-ratio math against known reference pairs
//   (b) that the committed token palette passes every asserted AA pairing
//       (a regression guard: any future token edit that breaks AA fails here,
//        not just in the standalone `npm run check:contrast` CI step)

import { describe, expect, it } from "vitest";

import {
  checkContrast,
  componentPairings,
  compositeOver,
  contrastRatio,
  enumeratePairings,
  relativeLuminance,
} from "../../../tools/design-tokens/contrast.js";

describe("contrastRatio (WCAG 2.1 reference pairs)", () => {
  it("black on white is the maximum 21:1", () => {
    expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 5);
  });

  it("is symmetric (order of the two colors does not matter)", () => {
    expect(contrastRatio("#000000", "#ffffff")).toBe(contrastRatio("#ffffff", "#000000"));
  });

  it("identical colors give the minimum 1:1", () => {
    expect(contrastRatio("#3d8b8b", "#3d8b8b")).toBeCloseTo(1, 5);
  });

  it("a mid-gray (#777777) on white is ~4.48:1 (the classic just-below-AA gray)", () => {
    // #777 is the textbook example that narrowly fails normal-text AA (4.5).
    expect(contrastRatio("#777777", "#ffffff")).toBeCloseTo(4.48, 2);
  });

  it("a known-failing pair: mid-gray on light gray is below 4.5:1", () => {
    const ratio = contrastRatio("#999999", "#dddddd");
    expect(ratio).toBeLessThan(4.5);
    expect(ratio).toBeCloseTo(2.1, 2);
  });

  it("accepts hex with or without a leading '#'", () => {
    expect(contrastRatio("000000", "ffffff")).toBeCloseTo(21, 5);
  });
});

describe("relativeLuminance (WCAG 2.1 endpoints)", () => {
  it("black is 0 and white is 1", () => {
    expect(relativeLuminance("#000000")).toBeCloseTo(0, 5);
    expect(relativeLuminance("#ffffff")).toBeCloseTo(1, 5);
  });

  it("green contributes more luminance than red, which contributes more than blue", () => {
    expect(relativeLuminance("#00ff00")).toBeGreaterThan(relativeLuminance("#ff0000"));
    expect(relativeLuminance("#ff0000")).toBeGreaterThan(relativeLuminance("#0000ff"));
  });
});

// A tiny synthetic palette: all-black foregrounds on white surfaces (so every
// token-derived pairing passes), except text.muted which fails 4.5:1 on white.
function syntheticTokens() {
  return {
    color: {
      primary: {
        default: { value: "#000000" },
        hover: { value: "#000000" },
        light: { value: "#ffffff" },
      },
      accent: { default: { value: "#000000" }, dark: { value: "#000000" } },
      surface: {
        bg: { value: "#ffffff" },
        "bg-warm": { value: "#ffffff" },
        default: { value: "#ffffff" },
        elevated: { value: "#ffffff" },
      },
      text: {
        default: { value: "#000000" },
        secondary: { value: "#000000" },
        // Fails 4.5:1 on white.
        muted: { value: "#aaaaaa" },
      },
      border: { default: { value: "#eeeeee" }, strong: { value: "#cccccc" } },
      feedback: {
        success: { value: "#000000" },
        warning: { value: "#000000" },
        error: { value: "#000000" },
        "error-light": { value: "#ffffff" },
      },
    },
  };
}

describe("enumeratePairings", () => {
  const tokens = syntheticTokens();

  it("includes a 4.5:1 normal-text assertion for muted text on each surface", () => {
    const mutedPairings = enumeratePairings(tokens).filter((p) =>
      p.name.startsWith("text.muted text on"),
    );
    expect(mutedPairings).toHaveLength(4);
    for (const p of mutedPairings) {
      expect(p.threshold).toBe(4.5);
      expect(p.kind).toBe("normal-text");
    }
  });

  it("does NOT assert on the resting decorative border tokens", () => {
    const borderPairings = enumeratePairings(tokens).filter((p) => p.name.includes("border."));
    expect(borderPairings).toHaveLength(0);
  });

  it("asserts fg-on-tint, white-on-fill, and focus-boundary pairings", () => {
    const names = enumeratePairings(tokens).map((p) => p.name);
    expect(names).toContain("primary.hover text on primary.light fill");
    expect(names).toContain("white text on primary.default fill");
    expect(names).toContain("white text on feedback.error fill");
    expect(names.some((n) => n.includes("focus boundary"))).toBe(true);
  });
});

describe("compositeOver (straight-alpha sRGB compositing)", () => {
  it("a fully opaque color is unchanged", () => {
    expect(compositeOver({ r: 18, g: 52, b: 86, a: 1 }, "#ffffff")).toBe("#123456");
  });

  it("a fully transparent color yields the base", () => {
    expect(compositeOver({ r: 255, g: 0, b: 0, a: 0 }, "#abcdef")).toBe("#abcdef");
  });

  it("50% black over white is mid-gray", () => {
    expect(compositeOver({ r: 0, g: 0, b: 0, a: 0.5 }, "#ffffff")).toBe("#808080");
  });
});

describe("componentPairings (rgba badge tints)", () => {
  it("composites the badge tint over each base surface and asserts 4.5:1", () => {
    const pairings = componentPairings(syntheticTokens());
    // 2 badge tints x 3 base surfaces.
    expect(pairings).toHaveLength(6);
    for (const p of pairings) {
      expect(p.threshold).toBe(4.5);
      expect(p.kind).toBe("normal-text");
    }
    expect(pairings.some((p) => p.name.includes(".op-badge-success"))).toBe(true);
    expect(pairings.some((p) => p.name.includes(".op-badge-error"))).toBe(true);
  });
});

describe("checkContrast on a synthetic palette", () => {
  it("flags a failing pairing and reports its ratio", () => {
    const { failures } = checkContrast(syntheticTokens());
    expect(failures.length).toBeGreaterThan(0);
    expect(failures.every((f) => f.name.startsWith("text.muted"))).toBe(true);
    expect(failures[0].ratio).toBeLessThan(4.5);
  });
});

describe("checkContrast on the committed token palette", () => {
  it("every asserted pairing meets its WCAG AA threshold", () => {
    // Loads docs/design/tokens.json via the checker's default. This is the
    // regression guard: a future token edit that breaks AA fails the suite.
    const { failures } = checkContrast();
    const report = failures
      .map((f) => `${f.name}: ${f.ratio.toFixed(2)}:1 (need ${f.threshold}:1)`)
      .join("\n");
    expect(failures, `WCAG AA failures:\n${report}`).toHaveLength(0);
  });
});
