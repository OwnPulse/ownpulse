// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

// Unit tests for the design-tokens build pipeline (tools/design-tokens/build.js).
//
// The generator is a Node/JS tool with no React surface, so it is exercised
// here in the web Vitest suite — the project's only JS unit-test runner. These
// tests lock in three properties:
//   (a) token path -> CSS variable name mapping matches the hand-written names
//   (b) value parity: a hex token renders identically in CSS and in Swift
//   (c) idempotency: a fresh build reproduces the committed files byte-for-byte
// Property (c) is what protects the future B3 drift gate from flapping.

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { buildTokens, cssVarName, swiftColor } from "../../../tools/design-tokens/build.js";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const cssPath = resolve(repoRoot, "web/src/styles/_tokens.css");
const swiftPath = resolve(repoRoot, "ios/OwnPulse/Theme/Tokens.swift");
const mdPath = resolve(repoRoot, "docs/design/tokens-generated.md");

const read = (p: string) => readFileSync(p, "utf8");

describe("cssVarName mapping", () => {
  it.each([
    [["color", "primary", "default"], "--color-primary"],
    [["color", "primary", "hover"], "--color-primary-hover"],
    [["color", "accent", "default"], "--color-accent"],
    [["color", "dimension", "gold"], "--color-gold"],
    [["color", "dimension", "sage"], "--color-sage"],
    [["color", "feedback", "success"], "--color-success"],
    [["color", "feedback", "error-light"], "--color-error-light"],
    [["color", "neutral", "900"], "--color-neutral-900"],
    [["color", "surface", "bg"], "--color-bg"],
    [["color", "surface", "bg-warm"], "--color-bg-warm"],
    [["color", "surface", "elevated"], "--color-surface-elevated"],
    [["color", "text", "default"], "--color-text"],
    [["color", "text", "muted"], "--color-text-muted"],
    [["color", "border", "strong"], "--color-border-strong"],
    [["typography", "font-family", "display"], "--font-display"],
    [["typography", "font-size", "xs"], "--text-xs"],
    [["spacing", "content-padding"], "--content-padding"],
    [["radii", "sm"], "--radius-sm"],
    [["shadow", "md"], "--shadow-md"],
  ])("maps %j -> %s", (path, expected) => {
    expect(cssVarName(path as string[])).toBe(expected);
  });

  it("omits the chart.metric group (consumed by B5, not the palette)", () => {
    expect(cssVarName(["chart", "metric", "heart_rate"])).toBeNull();
    expect(cssVarName(["chart", "metric", "fallback"])).toBeNull();
  });
});

describe("swiftColor", () => {
  it("renders n/255 fractional components matching the hand-written style", () => {
    expect(swiftColor("#c2654a")).toBe("Color(red: 194 / 255, green: 101 / 255, blue: 74 / 255)");
    expect(swiftColor("#ffffff")).toBe("Color(red: 255 / 255, green: 255 / 255, blue: 255 / 255)");
  });
});

describe("value parity (CSS <-> Swift)", () => {
  it("renders color.primary.default identically in both outputs", () => {
    // #b2573c in the source -> --color-primary: #b2573c in CSS,
    // and OPColor.terracotta as Color(red: 178/255, ...) in Swift.
    // (Darkened from #c2654a for WCAG AA — see tools/design-tokens/contrast.js.)
    expect(read(cssPath)).toContain("--color-primary: #b2573c;");
    expect(read(swiftPath)).toContain(
      "static let terracotta = Color(red: 178 / 255, green: 87 / 255, blue: 60 / 255)",
    );
  });

  it("renders color.accent.default identically in both outputs", () => {
    // Darkened from #3d8b8b for WCAG AA — see tools/design-tokens/contrast.js.
    expect(read(cssPath)).toContain("--color-accent: #377c7c;");
    expect(read(swiftPath)).toContain(
      "static let teal = Color(red: 55 / 255, green: 124 / 255, blue: 124 / 255)",
    );
  });
});

describe("idempotency", () => {
  it("a fresh build reproduces the committed files byte-for-byte", async () => {
    const before = {
      css: read(cssPath),
      swift: read(swiftPath),
      md: read(mdPath),
    };

    await buildTokens();

    expect(read(cssPath)).toBe(before.css);
    expect(read(swiftPath)).toBe(before.swift);
    expect(read(mdPath)).toBe(before.md);
  });
});
