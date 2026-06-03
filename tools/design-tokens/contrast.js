// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

// WCAG 2.1 contrast checker for the canonical design tokens.
//
// Reads docs/design/tokens.json, enumerates every text-on-background and
// graphical-object-on-background pairing the light-mode palette implies, and
// asserts each meets the relevant WCAG 2.1 minimum contrast ratio:
//   - normal text:                          >= 4.5:1  (1.4.3 AA)
//   - large text:                           >= 3:1    (1.4.3 AA)
//   - graphical objects / UI components:    >= 3:1    (1.4.11 AA)
//
// Run via `npm run check:contrast`. Exits non-zero on any failure with a
// report of the failing pair and its ratio. The dark-mode overrides in
// web/src/styles/variables.css are hand-written (not modeled in the token
// source), so they are out of scope here.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '..', '..');
const tokensPath = resolve(repoRoot, 'docs/design/tokens.json');

// WCAG 2.1 minimum contrast thresholds.
export const THRESHOLD_NORMAL_TEXT = 4.5;
export const THRESHOLD_LARGE_TEXT = 3.0;
export const THRESHOLD_GRAPHICAL = 3.0;

// --- contrast math (WCAG 2.1) ----------------------------------------------

// Parse a 6-digit hex color (with or without leading '#') to {r,g,b} in 0..255.
export function parseHex(hex) {
  const h = String(hex).replace('#', '').trim();
  if (!/^[0-9a-fA-F]{6}$/.test(h)) {
    throw new Error(`Not a 6-digit hex color: ${hex}`);
  }
  return {
    r: parseInt(h.slice(0, 2), 16),
    g: parseInt(h.slice(2, 4), 16),
    b: parseInt(h.slice(4, 6), 16),
  };
}

// Convert an sRGB 8-bit channel to its linearized value, per WCAG 2.1.
// https://www.w3.org/TR/WCAG21/#dfn-relative-luminance
function linearizeChannel(channel8bit) {
  const c = channel8bit / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

// Relative luminance of an sRGB color, per WCAG 2.1.
export function relativeLuminance(hex) {
  const { r, g, b } = parseHex(hex);
  return (
    0.2126 * linearizeChannel(r) +
    0.7152 * linearizeChannel(g) +
    0.0722 * linearizeChannel(b)
  );
}

// WCAG 2.1 contrast ratio between two colors: (L1 + 0.05) / (L2 + 0.05),
// where L1 is the lighter of the two relative luminances. Range 1..21.
// https://www.w3.org/TR/WCAG21/#dfn-contrast-ratio
export function contrastRatio(hexA, hexB) {
  const la = relativeLuminance(hexA);
  const lb = relativeLuminance(hexB);
  const lighter = Math.max(la, lb);
  const darker = Math.min(la, lb);
  return (lighter + 0.05) / (darker + 0.05);
}

// --- pairing enumeration ---------------------------------------------------

// Build the list of pairings the light-mode palette implies. Each entry is:
//   { name, fg, bg, threshold, kind }
// where `kind` is 'normal-text' | 'large-text' | 'graphical'.
export function enumeratePairings(tokens) {
  const c = tokens.color;
  const pairings = [];

  // Every opaque surface the palette can render content on top of.
  const surfaces = [
    ['surface.bg', c.surface.bg.value],
    ['surface.bg-warm', c.surface['bg-warm'].value],
    ['surface.default', c.surface.default.value],
    ['surface.elevated', c.surface.elevated.value],
  ];

  // Foreground text colors -> normal text against every surface.
  const textColors = [
    ['text.default', c.text.default.value],
    ['text.secondary', c.text.secondary.value],
    ['text.muted', c.text.muted.value],
  ];
  for (const [fgName, fg] of textColors) {
    for (const [bgName, bg] of surfaces) {
      pairings.push({
        name: `${fgName} text on ${bgName}`,
        fg,
        bg,
        threshold: THRESHOLD_NORMAL_TEXT,
        kind: 'normal-text',
      });
    }
  }

  // Interactive / branded text and icons (links, buttons-as-text, primary
  // accents) against every surface. Treated as normal text: they carry
  // meaning as text/glyphs, so the stricter 4.5:1 applies.
  const interactiveText = [
    ['primary.default', c.primary.default.value],
    ['primary.hover', c.primary.hover.value],
    ['accent.default', c.accent.default.value],
    ['accent.dark', c.accent.dark.value],
  ];
  for (const [fgName, fg] of interactiveText) {
    for (const [bgName, bg] of surfaces) {
      pairings.push({
        name: `${fgName} text on ${bgName}`,
        fg,
        bg,
        threshold: THRESHOLD_NORMAL_TEXT,
        kind: 'normal-text',
      });
    }
  }

  // Feedback colors used as text / status icons against every surface. Status
  // text must be readable, so normal-text 4.5:1 applies. error-light is a
  // BACKGROUND tint, not a foreground — handled separately below.
  const feedbackText = [
    ['feedback.success', c.feedback.success.value],
    ['feedback.warning', c.feedback.warning.value],
    ['feedback.error', c.feedback.error.value],
  ];
  for (const [fgName, fg] of feedbackText) {
    for (const [bgName, bg] of surfaces) {
      pairings.push({
        name: `${fgName} text on ${bgName}`,
        fg,
        bg,
        threshold: THRESHOLD_NORMAL_TEXT,
        kind: 'normal-text',
      });
    }
  }

  // error-light is the tint behind error text (e.g. inline error banners).
  // Body text and the error foreground sit on it, so both must clear normal
  // text against it.
  pairings.push({
    name: 'text.default on feedback.error-light',
    fg: c.text.default.value,
    bg: c.feedback['error-light'].value,
    threshold: THRESHOLD_NORMAL_TEXT,
    kind: 'normal-text',
  });
  pairings.push({
    name: 'feedback.error on feedback.error-light',
    fg: c.feedback.error.value,
    bg: c.feedback['error-light'].value,
    threshold: THRESHOLD_NORMAL_TEXT,
    kind: 'normal-text',
  });

  // Active / focus boundaries that COMMUNICATE STATE are graphical objects
  // under WCAG 1.4.11 (3:1). In the web CSS the focus/active ring uses
  // color.primary (e.g. `border-color: var(--color-primary)` on inputs, the
  // active tab underline). primary.default is already asserted above as normal
  // text at the stricter 4.5:1, so it transitively satisfies 3:1 here; we still
  // enumerate it explicitly so the checker documents the UI-component case.
  pairings.push(
    ...surfaces.map(([bgName, bg]) => ({
      name: `primary.default focus boundary on ${bgName}`,
      fg: c.primary.default.value,
      bg,
      threshold: THRESHOLD_GRAPHICAL,
      kind: 'graphical',
    })),
  );

  // NOTE: the resting border tokens (color.border.default / .strong) are
  // deliberately NOT asserted. They draw decorative card outlines, dividers,
  // and the resting edge of inputs that are themselves identified by fill,
  // label, and placeholder — i.e. boundaries "not required to identify" the
  // component, which WCAG 1.4.11 explicitly exempts. Forcing them to 3:1 would
  // darken every card edge and divider, a sweeping regression against the
  // intended soft palette. Their contrast is reported informationally below.

  return pairings;
}

// --- runner ----------------------------------------------------------------

function loadTokens() {
  return JSON.parse(readFileSync(tokensPath, 'utf8'));
}

// Evaluate every pairing. Returns { results, failures } where each result is
// { ...pairing, ratio, pass }.
export function checkContrast(tokens = loadTokens()) {
  const results = enumeratePairings(tokens).map((p) => {
    const ratio = contrastRatio(p.fg, p.bg);
    return { ...p, ratio, pass: ratio >= p.threshold };
  });
  const failures = results.filter((r) => !r.pass);
  return { results, failures };
}

function fmtRatio(r) {
  return `${r.toFixed(2)}:1`;
}

// Resting decorative borders are not asserted (see enumeratePairings), but we
// report their ratios so the numbers are visible and reviewable.
function informationalBorders(tokens) {
  const c = tokens.color;
  const surfaces = [
    ['surface.bg', c.surface.bg.value],
    ['surface.bg-warm', c.surface['bg-warm'].value],
    ['surface.elevated', c.surface.elevated.value],
  ];
  const borders = [
    ['border.default', c.border.default.value],
    ['border.strong', c.border.strong.value],
  ];
  const rows = [];
  for (const [fgName, fg] of borders) {
    for (const [bgName, bg] of surfaces) {
      rows.push({ name: `${fgName} on ${bgName}`, fg, bg, ratio: contrastRatio(fg, bg) });
    }
  }
  return rows;
}

// Run only when executed directly (`node contrast.js`), not when imported.
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const tokens = loadTokens();
  const { results, failures } = checkContrast(tokens);

  for (const r of results) {
    const status = r.pass ? 'PASS' : 'FAIL';
    console.log(
      `${status}  ${fmtRatio(r.ratio).padStart(7)}  (need ${fmtRatio(r.threshold)}, ${r.kind})  ${r.name}  [${r.fg} on ${r.bg}]`,
    );
  }

  console.log('\nInformational (decorative borders, not asserted — WCAG 1.4.11 exempt):');
  for (const b of informationalBorders(tokens)) {
    console.log(`INFO  ${fmtRatio(b.ratio).padStart(7)}  ${b.name}  [${b.fg} on ${b.bg}]`);
  }

  console.log('');
  if (failures.length > 0) {
    console.error(`WCAG AA contrast check FAILED: ${failures.length} pairing(s) below threshold:`);
    for (const f of failures) {
      console.error(
        `  - ${f.name}: ${fmtRatio(f.ratio)} (need ${fmtRatio(f.threshold)} for ${f.kind})  [${f.fg} on ${f.bg}]`,
      );
    }
    process.exit(1);
  }
  console.log(`WCAG AA contrast check passed: ${results.length} pairing(s) all meet threshold.`);
}
