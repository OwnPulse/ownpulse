// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

// Design-tokens build pipeline.
//
// Reads the canonical token source (docs/design/tokens.json) and generates
// reproducible outputs via Style Dictionary:
//   - web/src/styles/_tokens.css                            CSS custom properties (light :root +
//                                                            both dark-mode blocks, from the `dark.*` group)
//   - web/src/components/explore/chartMetricColors.generated.ts  per-metric chart colors (web)
//   - web/src/components/dimensionColors.generated.ts       check-in dimension colors (web)
//   - ios/OwnPulse/Theme/Tokens.swift                       OPColor.* + type/spacing/radii constants
//   - ios/OwnPulse/Theme/ChartColors.swift                  per-metric chart colors (iOS)
//   - docs/design/tokens-generated.md                       human-readable reference
//
// Running `npm run build:tokens` on a clean tree yields no diff.

import StyleDictionary from 'style-dictionary';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '..', '..');
const tokensPath = resolve(repoRoot, 'docs/design/tokens.json');

const HEADER_CSS = `/* SPDX-License-Identifier: AGPL-3.0-or-later */
/* GENERATED FILE — DO NOT EDIT BY HAND. */
/* Source: docs/design/tokens.json. Regenerate with \`npm run build:tokens\` in tools/design-tokens. */`;

const HEADER_SWIFT = `// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Source: docs/design/tokens.json. Regenerate with \`npm run build:tokens\` in tools/design-tokens.`;

const HEADER_TS = `// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Source: docs/design/tokens.json. Regenerate with \`npm run build:tokens\` in tools/design-tokens.`;

// --- helpers ---------------------------------------------------------------

function hexToRgbComponents(hex) {
  const h = hex.replace('#', '');
  const r = parseInt(h.slice(0, 2), 16);
  const g = parseInt(h.slice(2, 4), 16);
  const b = parseInt(h.slice(4, 6), 16);
  return { r, g, b };
}

// Swift Color initializer from a hex string, matching the hand-written style
// previously used in Theme.swift (red:/green:/blue: as n/255 fractions).
export function swiftColor(hex) {
  const { r, g, b } = hexToRgbComponents(hex);
  return `Color(red: ${r} / 255, green: ${g} / 255, blue: ${b} / 255)`;
}

// Maps the flat token path (array of segments) to a kebab-case CSS variable
// name that matches the existing hand-written variables.css naming. Returns
// null for tokens that are not surfaced as CSS variables — notably the entire
// `chart.metric.*` group, which is consumed by the per-metric chart-color unit
// (B5), not the palette, so it deliberately falls through to the `default`
// case and is omitted from _tokens.css.
export function cssVarName(path) {
  const [group, ...rest] = path;
  switch (group) {
    case 'color': {
      // color.primary.default -> --color-primary
      // color.primary.hover   -> --color-primary-hover
      // color.neutral.900     -> --color-neutral-900
      // color.surface.bg-warm -> --color-bg-warm
      // color.text.default    -> --color-text
      if (rest[0] === 'surface') {
        // surface.bg / bg-warm / default / elevated
        const sub = rest[1];
        if (sub === 'bg') return '--color-bg';
        if (sub === 'bg-warm') return '--color-bg-warm';
        if (sub === 'default') return '--color-surface';
        if (sub === 'elevated') return '--color-surface-elevated';
        return null;
      }
      if (rest[0] === 'text') {
        const sub = rest[1];
        if (sub === 'default') return '--color-text';
        return `--color-text-${sub}`;
      }
      if (rest[0] === 'dimension' || rest[0] === 'feedback') {
        // dimension.gold -> --color-gold; feedback.success -> --color-success
        return `--color-${rest[1]}`;
      }
      const segs = rest.filter((s) => s !== 'default');
      return `--color-${[rest[0], ...segs.slice(1)].join('-')}`;
    }
    case 'typography': {
      if (rest[0] === 'font-family') {
        // font-family.display -> --font-display
        return `--font-${rest[1]}`;
      }
      if (rest[0] === 'font-size') {
        // font-size.xs -> --text-xs
        return `--text-${rest[1]}`;
      }
      return null;
    }
    case 'spacing': {
      // spacing.content-padding -> --content-padding
      return `--${rest.join('-')}`;
    }
    case 'radii': {
      // radii.sm -> --radius-sm
      return `--radius-${rest.join('-')}`;
    }
    case 'shadow': {
      // shadow.sm -> --shadow-sm
      return `--shadow-${rest.join('-')}`;
    }
    default:
      return null;
  }
}

// The iOS theme exposes a curated, semantic subset of the palette via OPColor.
// Map token path -> Swift constant name. Only these are surfaced; the rest of
// the palette is web-only chrome.
const SWIFT_COLOR_MAP = {
  'color.primary.default': 'terracotta',
  'color.accent.default': 'teal',
  'color.dimension.gold': 'gold',
  'color.dimension.sage': 'sage',
  'color.dimension.purple': 'purple',
  'color.dimension.energy': 'dimensionEnergy',
  'color.dimension.mood': 'dimensionMood',
  'color.dimension.focus': 'dimensionFocus',
  'color.dimension.recovery': 'dimensionRecovery',
  'color.dimension.libido': 'dimensionLibido',
  'color.surface.bg-warm': 'warmBg',
  'color.surface.elevated': 'cardLight',
  'color.feedback.success': 'success',
  'color.feedback.warning': 'warning',
  'color.feedback.error': 'error',
  'dark.color.bg': 'darkBg',
  'dark.color.surface-elevated': 'cardDark',
};

// Maps backend `record_type` field strings (as emitted by the explore API —
// see backend/api/src/models/explore.rs `HealthRecordField::record_type`) to
// the canonical chart.metric token key. Only synonyms need an entry: a field
// whose name already equals its token key (e.g. `heart_rate`) is omitted.
//
// This table is the single source of truth for the alias layer and is emitted
// to BOTH the web (chartMetricColors.generated.ts) and iOS (ChartColors.swift)
// lookups, so the same metric resolves to the same color on both platforms and
// the alias logic can never drift between them. Every value here MUST be a key
// in the chart.metric token group (validated in chartMetricColors below).
// The check-in subjective scores (energy/mood/focus/recovery/libido) are
// deliberately NOT keyed here: they have no dedicated token color, so they
// fall through to the fallback cycle and are distinguished by their position
// index instead (see SparklineCard / DashboardView.sparklineSection).
export const METRIC_FIELD_ALIASES = {
  heart_rate_variability: 'hrv',
  resting_heart_rate: 'heart_rate',
  blood_pressure_systolic: 'bp_systolic',
  blood_pressure_diastolic: 'bp_diastolic',
  blood_glucose: 'glucose',
  body_mass: 'weight',
  sleep_analysis: 'sleep_duration',
};

// Extracts the per-metric chart-color group from the token dictionary as a
// plain object: { metrics: { heart_rate: '#d55e00', ... }, fallback: [...],
// aliases: { body_mass: 'weight', ... } }. This is the single source of truth
// for both the web keyed lookup (chartColors.ts) and the iOS keyed lookup
// (ChartColors.swift), so the same metric resolves to the same color on both
// platforms (B5 parity).
export function chartMetricColors(dictionary) {
  const metrics = {};
  let fallback = null;
  for (const token of dictionary.allTokens) {
    const [group, sub, key] = token.path;
    if (group !== 'chart' || sub !== 'metric') continue;
    if (key === 'fallback') {
      fallback = token.original.value;
    } else {
      metrics[key] = token.original.value;
    }
  }
  if (!fallback) throw new Error('chart.metric.fallback missing from token source');

  // Deterministic key order so generated output is stable regardless of how
  // Style Dictionary orders the dictionary.
  const ordered = {};
  for (const k of Object.keys(metrics).sort()) ordered[k] = metrics[k];

  // Every alias target must be a real token key, otherwise the alias is dead.
  for (const [field, key] of Object.entries(METRIC_FIELD_ALIASES)) {
    if (!(key in ordered)) {
      throw new Error(`alias ${field} -> ${key} targets a missing chart.metric key`);
    }
  }
  const aliases = {};
  for (const k of Object.keys(METRIC_FIELD_ALIASES).sort()) {
    aliases[k] = METRIC_FIELD_ALIASES[k];
  }

  return { metrics: ordered, fallback, aliases };
}

// The five check-in subjective-score dimensions, in canonical display order.
// Each mirrors an existing brand token (see the $description on each
// color.dimension.* entry in tokens.json) rather than introducing new hues.
export const DIMENSION_KEYS = ['energy', 'mood', 'focus', 'recovery', 'libido'];

// Extracts the check-in dimension colors from the token dictionary as a plain
// object keyed lowercase: { energy: '#c49a3c', mood: '#b2573c', ... }. Single
// source of truth for the web `DIMENSION_COLORS` map (dimensionColors.generated.ts)
// so CheckinForm, ScoreRing, and SparklineRow can no longer drift from each other.
export function dimensionColors(dictionary) {
  const byPath = new Map(dictionary.allTokens.map((t) => [t.path.join('.'), t]));
  const ordered = {};
  for (const k of DIMENSION_KEYS) {
    const token = byPath.get(`color.dimension.${k}`);
    if (!token) throw new Error(`color.dimension.${k} missing from token source`);
    ordered[k] = token.original.value;
  }
  return ordered;
}

// Extracts the intervention marker color (chart.intervention) from the token
// dictionary. Single source of truth for INTERVENTION_COLOR re-exported from
// both chartMetricColors.generated.ts and chartColors.ts.
export function interventionColor(dictionary) {
  const token = dictionary.allTokens.find((t) => t.path.join('.') === 'chart.intervention');
  if (!token) throw new Error('chart.intervention missing from token source');
  return token.original.value;
}

// Dark-mode override keys, in the order the two CSS blocks below emit them
// (matches the order the hand-written variables.css blocks used to use).
// Keyed to match the CSS custom-property name they override, e.g. 'bg' ->
// --color-bg.
export const DARK_COLOR_KEYS = [
  'link',
  'link-hover',
  'success-fg',
  'bg',
  'bg-warm',
  'surface',
  'surface-elevated',
  'text',
  'text-secondary',
  'text-muted',
  'border',
  'border-strong',
  'error-light',
  'primary-light',
];
export const DARK_SHADOW_KEYS = ['sm', 'md', 'lg'];

// Extracts the dark-mode override group from the token dictionary as
// { color: { bg: '#1a1a18', ... }, shadow: { sm: '...', ... } }, keyed to the
// CSS custom-property name each overrides. Single source of truth for BOTH
// dark-mode CSS blocks in _tokens.css ([data-theme="dark"] and the
// prefers-color-scheme media query) — those used to be two hand-maintained
// blocks in variables.css, kept in sync only by copy-paste discipline.
export function darkTokens(dictionary) {
  const byPath = new Map(dictionary.allTokens.map((t) => [t.path.join('.'), t]));
  const color = {};
  for (const k of DARK_COLOR_KEYS) {
    const token = byPath.get(`dark.color.${k}`);
    if (!token) throw new Error(`dark.color.${k} missing from token source`);
    color[k] = token.original.value;
  }
  const shadow = {};
  for (const k of DARK_SHADOW_KEYS) {
    const token = byPath.get(`dark.shadow.${k}`);
    if (!token) throw new Error(`dark.shadow.${k} missing from token source`);
    shadow[k] = token.original.value;
  }
  return { color, shadow };
}

// --- custom CSS format -----------------------------------------------------

StyleDictionary.registerFormat({
  name: 'ownpulse/css-tokens',
  format: ({ dictionary }) => {
    const lines = [];
    for (const token of dictionary.allTokens) {
      const name = cssVarName(token.path);
      if (!name) continue;
      lines.push({ name, value: token.original.value, path: token.path });
    }

    // Order follows Style Dictionary's `dictionary.allTokens`, which sorts
    // numeric-like keys ascending (so neutrals render 50 -> 900 even though the
    // source lists them 900 -> 50). It is deterministic, not source-order;
    // either way CSS custom-property order is irrelevant to resolution.
    const body = lines.map((l) => `  ${l.name}: ${l.value};`).join('\n');

    const { color: darkColor, shadow: darkShadow } = darkTokens(dictionary);
    const darkDecls = [
      ...Object.entries(darkColor).map(([k, v]) => [`--color-${k}`, v]),
      ...Object.entries(darkShadow).map(([k, v]) => [`--shadow-${k}`, v]),
    ];
    const darkBlock = darkDecls.map(([name, v]) => `  ${name}: ${v};`).join('\n');
    const darkBlockIndented = darkDecls.map(([name, v]) => `    ${name}: ${v};`).join('\n');

    return `${HEADER_CSS}

:root {
${body}
}

/* Explicit dark mode. Values from the \`dark.*\` group in docs/design/tokens.json.
   Selector is :root[data-theme="dark"], not the bare attribute selector,
   because this file is @imported at the top of variables.css: that import
   ordering puts these declarations BEFORE variables.css's own plain :root
   rules (e.g. --color-link's light-mode default) in source order. A bare
   [data-theme="dark"] would tie that plain :root rule on specificity and
   lose to it (later wins on a tie) in explicit dark mode. Qualifying with
   :root raises specificity so this block wins regardless of import order —
   correct anyway, since data-theme is only ever set on documentElement
   (:root) — see web/src/hooks/useTheme.ts. */
:root[data-theme="dark"] {
${darkBlock}
}

/* System preference auto-detection (when no explicit theme is set). */
@media (prefers-color-scheme: dark) {
  :root:not([data-theme]) {
${darkBlockIndented}
  }
}
`;
  },
});

// --- custom Swift format ---------------------------------------------------

StyleDictionary.registerFormat({
  name: 'ownpulse/swift-tokens',
  format: ({ dictionary }) => {
    const byPath = new Map(dictionary.allTokens.map((t) => [t.path.join('.'), t]));

    // OPColor palette subset.
    const colorLines = Object.entries(SWIFT_COLOR_MAP).map(([path, name]) => {
      const token = byPath.get(path);
      if (!token) throw new Error(`Swift color token missing from source: ${path}`);
      return `    static let ${name} = ${swiftColor(token.original.value)}`;
    });

    // Spacing / radii as CGFloat (px values only; rem/string layout values are web-only).
    const radii = ['sm', 'md', 'lg']
      .map((k) => {
        const t = byPath.get(`radii.${k}`);
        const px = parseInt(String(t.original.value).replace('px', ''), 10);
        return `    static let ${k}: CGFloat = ${px}`;
      })
      .join('\n');

    // Type scale: rem -> pt at the 16pt base, matching the web 1rem = 16px convention.
    const typeLines = ['xs', 'sm', 'base', 'lg', 'xl', '2xl', '3xl', '4xl']
      .map((k) => {
        const t = byPath.get(`typography.font-size.${k}`);
        const rem = parseFloat(String(t.original.value).replace('rem', ''));
        const pt = Math.round(rem * 16 * 100) / 100;
        const swiftKey = /^[0-9]/.test(k) ? `size${k}` : k;
        return `    static let ${swiftKey}: CGFloat = ${pt}`;
      })
      .join('\n');

    return `${HEADER_SWIFT}

import SwiftUI

/// Brand color palette, generated from the canonical token source.
enum OPColor {
${colorLines.join('\n')}
}

/// Corner radii, generated from the canonical token source.
enum OPRadius {
${radii}
}

/// Type scale in points (1rem == 16pt), generated from the canonical token source.
enum OPFontSize {
${typeLines}
}
`;
  },
});

// --- custom chart-metric TS format (web) -----------------------------------

StyleDictionary.registerFormat({
  name: 'ownpulse/chart-metric-ts',
  format: ({ dictionary }) => {
    const { metrics, fallback, aliases } = chartMetricColors(dictionary);
    // Emit object keys unquoted when they are valid JS identifiers, matching
    // the project's Biome formatter (which strips unnecessary quotes).
    const tsKey = (k) => (/^[A-Za-z_$][\w$]*$/.test(k) ? k : JSON.stringify(k));
    const metricLines = Object.entries(metrics)
      .map(([k, v]) => `  ${tsKey(k)}: ${JSON.stringify(v)},`)
      .join('\n');
    const fallbackLines = fallback.map((v) => `  ${JSON.stringify(v)},`).join('\n');
    const aliasLines = Object.entries(aliases)
      .map(([k, v]) => `  ${tsKey(k)}: ${JSON.stringify(v)},`)
      .join('\n');
    const intervention = interventionColor(dictionary);
    return `${HEADER_TS}

/** Per-metric chart colors, keyed by canonical metric name. */
export const METRIC_COLORS: Record<string, string> = {
${metricLines}
};

/** Deterministic fallback cycle for metrics without a dedicated color. */
export const FALLBACK_COLORS: readonly string[] = [
${fallbackLines}
];

/** Backend \`record_type\` field names that are synonyms for a canonical metric key. */
export const METRIC_ALIASES: Record<string, string> = {
${aliasLines}
};

/** Marker color for intervention (substance/medication/supplement) events overlaid on charts. */
export const INTERVENTION_COLOR: string = ${JSON.stringify(intervention)};
`;
  },
});

// --- custom dimension-color TS format (web) --------------------------------

StyleDictionary.registerFormat({
  name: 'ownpulse/dimension-colors-ts',
  format: ({ dictionary }) => {
    const colors = dimensionColors(dictionary);
    const unionType = DIMENSION_KEYS.map((k) => JSON.stringify(k)).join(' | ');
    const mapLines = Object.entries(colors)
      .map(([k, v]) => `  ${k}: ${JSON.stringify(v)},`)
      .join('\n');
    return `${HEADER_TS}

/** The five check-in subjective-score dimensions. */
export type DimensionName = ${unionType};

/**
 * Check-in dimension colors, keyed lowercase by dimension name. Single source
 * of truth for CheckinForm, ScoreRing, and SparklineRow so the five dimension
 * colors cannot drift between components.
 */
export const DIMENSION_COLORS: Record<DimensionName, string> = {
${mapLines}
};
`;
  },
});

// --- custom chart-metric Swift format (iOS) --------------------------------

StyleDictionary.registerFormat({
  name: 'ownpulse/chart-metric-swift',
  format: ({ dictionary }) => {
    const { metrics, fallback, aliases } = chartMetricColors(dictionary);
    const metricLines = Object.entries(metrics)
      .map(([k, v]) => `        ${JSON.stringify(k)}: ${swiftColor(v)},`)
      .join('\n');
    const fallbackLines = fallback.map((v) => `        ${swiftColor(v)},`).join('\n');
    const aliasLines = Object.entries(aliases)
      .map(([k, v]) => `        ${JSON.stringify(k)}: ${JSON.stringify(v)},`)
      .join('\n');
    return `${HEADER_SWIFT}

import SwiftUI

/// Per-metric chart colors, generated from the canonical token source.
/// Shares its source of truth with the web \`chartColors.ts\` map (including the
/// field-name alias layer), so the same metric resolves to the same color on
/// both platforms.
enum ChartColors {
    /// Colors keyed by canonical metric name.
    static let metric: [String: Color] = [
${metricLines}
    ]

    /// Deterministic fallback cycle for metrics without a dedicated color.
    static let fallback: [Color] = [
${fallbackLines}
    ]

    /// Backend \`record_type\` field names that are synonyms for a canonical key.
    static let aliases: [String: String] = [
${aliasLines}
    ]

    /// Resolves a metric to its color: the keyed color when the field (or one
    /// of its aliases) has one, otherwise the fallback cycle indexed by \`index\`.
    static func color(for metric: String, index: Int) -> Color {
        let key = aliases[metric] ?? metric
        if let mapped = self.metric[key] {
            return mapped
        }
        return fallback[((index % fallback.count) + fallback.count) % fallback.count]
    }
}
`;
  },
});

// --- custom Markdown format ------------------------------------------------

StyleDictionary.registerFormat({
  name: 'ownpulse/markdown-tokens',
  format: ({ dictionary }) => {
    const rows = [];
    for (const token of dictionary.allTokens) {
      const path = token.path.join('.');
      const value = token.original.value;
      const rendered = Array.isArray(value) ? value.join(', ') : value;
      rows.push(`| \`${path}\` | \`${rendered}\` |`);
    }
    return `<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- GENERATED FILE — DO NOT EDIT BY HAND. -->

# OwnPulse Design Tokens (Generated Reference)

Source of truth: [\`docs/design/tokens.json\`](./tokens.json).
Regenerate with \`npm run build:tokens\` in \`tools/design-tokens\`.

| Token | Value |
| ----- | ----- |
${rows.join('\n')}
`;
  },
});

// --- run -------------------------------------------------------------------

// Builds all three platforms and writes the generated files to their committed
// locations. Exported so tests (and a future drift gate) can invoke the exact
// same pipeline the `build:tokens` script runs.
export async function buildTokens() {
  const sd = new StyleDictionary({
    source: [tokensPath],
    platforms: {
      css: {
        transformGroup: 'css',
        buildPath: resolve(repoRoot, 'web/src/styles') + '/',
        files: [{ destination: '_tokens.css', format: 'ownpulse/css-tokens' }],
      },
      chartTs: {
        transformGroup: 'js',
        buildPath: resolve(repoRoot, 'web/src/components/explore') + '/',
        files: [{ destination: 'chartMetricColors.generated.ts', format: 'ownpulse/chart-metric-ts' }],
      },
      dimensionTs: {
        transformGroup: 'js',
        buildPath: resolve(repoRoot, 'web/src/components') + '/',
        files: [{ destination: 'dimensionColors.generated.ts', format: 'ownpulse/dimension-colors-ts' }],
      },
      swift: {
        transformGroup: 'js',
        buildPath: resolve(repoRoot, 'ios/OwnPulse/Theme') + '/',
        files: [
          { destination: 'Tokens.swift', format: 'ownpulse/swift-tokens' },
          { destination: 'ChartColors.swift', format: 'ownpulse/chart-metric-swift' },
        ],
      },
      docs: {
        transformGroup: 'js',
        buildPath: resolve(repoRoot, 'docs/design') + '/',
        files: [{ destination: 'tokens-generated.md', format: 'ownpulse/markdown-tokens' }],
      },
    },
  });

  await sd.hasInitialized;
  await sd.buildAllPlatforms();
}

// Resolved token dictionary with no platform-specific transforms applied,
// for tests exercising dimensionColors()/interventionColor()/chartMetricColors()
// against the real token source rather than a hand-rolled fixture.
export async function loadDictionary() {
  const sd = new StyleDictionary({
    source: [tokensPath],
    platforms: { js: { transformGroup: 'js' } },
  });
  await sd.hasInitialized;
  return sd.getPlatformTokens('js');
}

// Run only when executed directly (`node build.js`), not when imported by a test.
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  await buildTokens();
  console.log('Design tokens built.');
}
