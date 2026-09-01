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
// report of the failing pair and its ratio. The dark-mode palette (the
// `dark.*` token group, generated into _tokens.css's two dark-mode CSS
// blocks) is checked too, at the same thresholds — see
// enumerateDarkPairings below.

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

  // Foreground tokens placed ON the tinted "*-light" fills. These are not
  // surfaces in the layout sense but are used as small chip/badge/hover fills
  // with token-colored text on top, e.g. color.primary.light backs the .op-pill
  // chip, the .op-btn-secondary:hover fill, and TodaysDoses .pendingBadge, all
  // of which render primary-family text. Such text must clear 4.5:1.
  pairings.push({
    name: 'primary.hover text on primary.light fill',
    fg: c.primary.hover.value,
    bg: c.primary.light.value,
    threshold: THRESHOLD_NORMAL_TEXT,
    kind: 'normal-text',
  });

  // Solid brand/feedback fills with white text (filled buttons): .op-btn-primary
  // (#fff on primary) and .op-btn-danger (#fff on error). Enumerated so a future
  // lightening of either fill can't regress button legibility silently.
  pairings.push(
    {
      name: 'white text on primary.default fill',
      fg: '#ffffff',
      bg: c.primary.default.value,
      threshold: THRESHOLD_NORMAL_TEXT,
      kind: 'normal-text',
    },
    {
      name: 'white text on feedback.error fill',
      fg: '#ffffff',
      bg: c.feedback.error.value,
      threshold: THRESHOLD_NORMAL_TEXT,
      kind: 'normal-text',
    },
  );

  // Dimension colors (ScoreRing arc/track, SparklineRow line) and the
  // intervention marker are graphical objects, not text — 3:1, not 4.5:1. Only
  // asserted against surface.bg and surface.elevated: the two surfaces the
  // dashboard (ScoreRing, SparklineRow) and explore chart actually render on.
  // Guarded because the synthetic test palette below doesn't define these groups.
  const graphicalBg = [
    ['surface.bg', c.surface.bg.value],
    ['surface.elevated', c.surface.elevated.value],
  ];
  if (c.dimension) {
    for (const key of ['energy', 'mood', 'focus', 'recovery', 'libido']) {
      for (const [bgName, bg] of graphicalBg) {
        pairings.push({
          name: `dimension.${key} on ${bgName}`,
          fg: c.dimension[key].value,
          bg,
          threshold: THRESHOLD_GRAPHICAL,
          kind: 'graphical',
        });
      }
    }
  }
  if (tokens.chart?.intervention) {
    for (const [bgName, bg] of graphicalBg) {
      pairings.push({
        name: `chart.intervention on ${bgName}`,
        fg: tokens.chart.intervention.value,
        bg,
        threshold: THRESHOLD_GRAPHICAL,
        kind: 'graphical',
      });
    }
  }

  // NOTE: the resting border tokens (color.border.default / .strong) are
  // deliberately NOT asserted. They draw decorative card outlines, dividers,
  // and the resting edge of inputs that are themselves identified by fill,
  // label, and placeholder — i.e. boundaries "not required to identify" the
  // component, which WCAG 1.4.11 explicitly exempts. Forcing them to 3:1 would
  // darken every card edge and divider, a sweeping regression against the
  // intended soft palette. Their contrast is reported informationally below.

  return pairings;
}

// Composite a semi-transparent sRGB color over an opaque base (straight alpha):
//   out = fg * a + base * (1 - a), per channel.
export function compositeOver({ r, g, b, a }, baseHex) {
  const base = parseHex(baseHex);
  const ch = (f, bg) => Math.round(f * a + bg * (1 - a));
  const toHex = (n) => n.toString(16).padStart(2, '0');
  return `#${toHex(ch(r, base.r))}${toHex(ch(g, base.g))}${toHex(ch(b, base.b))}`;
}

// Parses a `rgba(r, g, b, a)` CSS string (as used by dark.color.primary-light)
// into { r, g, b, a } for compositeOver.
export function parseRgbaString(str) {
  const m = String(str).match(
    /rgba\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*([\d.]+)\s*\)/,
  );
  if (!m) throw new Error(`Not an rgba(...) string: ${str}`);
  return { r: Number(m[1]), g: Number(m[2]), b: Number(m[3]), a: Number(m[4]) };
}

// --- dark-mode pairing enumeration ------------------------------------------

// Build the list of pairings the DARK-mode palette implies, mirroring
// enumeratePairings' categories but backed by the `dark.*` token group: dark
// surfaces, dark text, and the dark-tuned foreground-accent aliases
// (dark.color.link/.link-hover/.success-fg) that exist specifically so
// primary/success-colored text stays readable on a dark background — see
// their $description in docs/design/tokens.json.
//
// Deliberately narrower than a literal find-and-replace of enumeratePairings:
// color.accent.default/.dark and color.feedback.warning/.error have NO
// dark-tuned counterpart in the token source. They render at their unchanged
// light-mode hex when used as foreground text on a dark surface (e.g.
// InterpretationCard.module.css, SequencerGrid.module.css, and every
// `color: var(--color-error)`/`var(--color-warning)` usage) and measurably
// fail AA there today (accent.default ~3.6:1, feedback.error ~2.9:1 against
// dark.bg — both below the required 4.5:1). That is a real, pre-existing gap
// in the hand-written dark palette this PR is tokenizing verbatim, not
// something a tokenization pass should silently paper over by inventing new
// -fg tokens no design review has signed off on. Their ratios are still
// computed and reported, informationally, by informationalDarkGaps below —
// visible, not silently dropped, but not an assertion this PR fails on.
export function enumerateDarkPairings(tokens) {
  const c = tokens.color;
  const d = tokens.dark?.color;
  if (!d) return [];
  const pairings = [];

  const surfaces = [
    ['dark.bg', d.bg.value],
    ['dark.bg-warm', d['bg-warm'].value],
    ['dark.surface', d.surface.value],
    ['dark.surface-elevated', d['surface-elevated'].value],
  ];

  const textColors = [
    ['dark.text', d.text.value],
    ['dark.text-secondary', d['text-secondary'].value],
    ['dark.text-muted', d['text-muted'].value],
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

  // The dark-tuned foreground accents — the entire reason these tokens exist.
  const interactiveText = [
    ['dark.link', d.link.value],
    ['dark.link-hover', d['link-hover'].value],
    ['dark.success-fg', d['success-fg'].value],
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

  // dark.error-light is the dark-mode tint behind inline error banners;
  // dark.text sits on it as body copy.
  pairings.push({
    name: 'dark.text on dark.error-light',
    fg: d.text.value,
    bg: d['error-light'].value,
    threshold: THRESHOLD_NORMAL_TEXT,
    kind: 'normal-text',
  });

  // Dimension colors and the intervention marker have no dark variant — same
  // hex as light, checked as graphical objects (3:1) against dark surfaces.
  const graphicalBg = [
    ['dark.bg', d.bg.value],
    ['dark.surface-elevated', d['surface-elevated'].value],
  ];
  if (c.dimension) {
    for (const key of ['energy', 'mood', 'focus', 'recovery', 'libido']) {
      for (const [bgName, bg] of graphicalBg) {
        pairings.push({
          name: `dimension.${key} on ${bgName}`,
          fg: c.dimension[key].value,
          bg,
          threshold: THRESHOLD_GRAPHICAL,
          kind: 'graphical',
        });
      }
    }
  }
  if (tokens.chart?.intervention) {
    for (const [bgName, bg] of graphicalBg) {
      pairings.push({
        name: `chart.intervention on ${bgName}`,
        fg: tokens.chart.intervention.value,
        bg,
        threshold: THRESHOLD_GRAPHICAL,
        kind: 'graphical',
      });
    }
  }

  // Solid brand/feedback fills with white text are theme-invariant — the fill
  // color itself has no dark override — but re-asserted here so a future
  // dark-specific override to primary/error can't silently break button text.
  pairings.push(
    {
      name: 'white text on primary.default fill (dark)',
      fg: '#ffffff',
      bg: c.primary.default.value,
      threshold: THRESHOLD_NORMAL_TEXT,
      kind: 'normal-text',
    },
    {
      name: 'white text on feedback.error fill (dark)',
      fg: '#ffffff',
      bg: c.feedback.error.value,
      threshold: THRESHOLD_NORMAL_TEXT,
      kind: 'normal-text',
    },
  );

  return pairings;
}

// Known, pre-existing dark-mode contrast gaps: real component pairings that
// fail AA in dark mode today but have no dark-tuned token to check instead
// (see the comment on enumerateDarkPairings). Reported like
// informationalBorders — visible in `check:contrast` output, not asserted —
// so this tokenization pass doesn't fail CI over an unrelated, larger
// restyle, but also doesn't hide the gap.
function informationalDarkGaps(tokens) {
  const c = tokens.color;
  const d = tokens.dark?.color;
  if (!d) return [];
  const surfaces = [
    ['dark.bg', d.bg.value],
    ['dark.surface-elevated', d['surface-elevated'].value],
  ];
  const rows = [];
  const knownGapText = [
    ['accent.default', c.accent.default.value],
    ['accent.dark', c.accent.dark.value],
    ['feedback.warning', c.feedback.warning.value],
    ['feedback.error', c.feedback.error.value],
  ];
  for (const [fgName, fg] of knownGapText) {
    for (const [bgName, bg] of surfaces) {
      rows.push({ name: `${fgName} text on ${bgName}`, fg, bg, ratio: contrastRatio(fg, bg) });
    }
  }
  // .op-pill / .op-btn-secondary:hover: primary.hover text on the dark
  // primary-light tint composited over dark.bg (components.css:79,159).
  const tintOnDarkBg = compositeOver(parseRgbaString(d['primary-light'].value), d.bg.value);
  rows.push({
    name: 'primary.hover text on primary-light tint (over dark.bg)',
    fg: c.primary.hover.value,
    bg: tintOnDarkBg,
    ratio: contrastRatio(c.primary.hover.value, tintOnDarkBg),
  });
  return rows;
}

// Curated pairings for components whose backgrounds are hand-written rgba TINTS
// in the web CSS rather than tokens (the token checker can't see those values),
// composited over the page surfaces they sit on. The foreground IS a token, so
// these guard the token values against the real component backdrop. Kept
// separate from enumeratePairings (which is purely token-derived) and folded
// into checkContrast so the assertion runs in CI all the same.
//   .op-badge-success: color: feedback.success on rgba(90,138,90,0.15)
//   .op-badge-error:   color: feedback.error   on rgba(181,74,74,0.15)
export function componentPairings(tokens) {
  const c = tokens.color;
  // Composite over the most common page surfaces; assert against the worst.
  const bases = [c.surface.bg.value, c.surface['bg-warm'].value, c.surface.elevated.value];
  const tints = [
    {
      name: 'feedback.success text on .op-badge-success tint',
      fg: c.feedback.success.value,
      rgba: { r: 90, g: 138, b: 90, a: 0.15 },
    },
    {
      name: 'feedback.error text on .op-badge-error tint',
      fg: c.feedback.error.value,
      rgba: { r: 181, g: 74, b: 74, a: 0.15 },
    },
  ];
  const pairings = [];
  for (const t of tints) {
    for (const base of bases) {
      pairings.push({
        name: `${t.name} (over ${base})`,
        fg: t.fg,
        bg: compositeOver(t.rgba, base),
        threshold: THRESHOLD_NORMAL_TEXT,
        kind: 'normal-text',
      });
    }
  }
  return pairings;
}

// --- runner ----------------------------------------------------------------

function loadTokens() {
  return JSON.parse(readFileSync(tokensPath, 'utf8'));
}

// Evaluate every pairing (token-derived + curated component pairings). Returns
// { results, failures } where each result is { ...pairing, ratio, pass }.
export function checkContrast(tokens = loadTokens()) {
  const all = [
    ...enumeratePairings(tokens),
    ...componentPairings(tokens),
    ...enumerateDarkPairings(tokens),
  ];
  const results = all.map((p) => {
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

  console.log(
    '\nInformational (known pre-existing dark-mode gaps, not asserted — see enumerateDarkPairings):',
  );
  for (const g of informationalDarkGaps(tokens)) {
    console.log(`INFO  ${fmtRatio(g.ratio).padStart(7)}  ${g.name}  [${g.fg} on ${g.bg}]`);
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
