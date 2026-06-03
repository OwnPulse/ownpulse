// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

// Design-tokens build pipeline.
//
// Reads the canonical token source (docs/design/tokens.json) and generates
// three reproducible outputs via Style Dictionary:
//   - web/src/styles/_tokens.css      CSS custom properties
//   - ios/OwnPulse/Theme/Tokens.swift OPColor.* + type/spacing/radii constants
//   - docs/design/tokens-generated.md human-readable reference
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
function swiftColor(hex) {
  const { r, g, b } = hexToRgbComponents(hex);
  return `Color(red: ${r} / 255, green: ${g} / 255, blue: ${b} / 255)`;
}

// Maps the flat token path (dot-joined) to a kebab-case CSS variable name that
// matches the existing hand-written variables.css naming. Returns null for
// tokens that are not surfaced as CSS variables (chart.metric is consumed by
// the chart-color unit, not the palette).
function cssVarName(path) {
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
  'color.surface.bg-warm': 'warmBg',
  'color.surface.elevated': 'cardLight',
};

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

    // Stable, grouped ordering driven by the source token order.
    const body = lines.map((l) => `  ${l.name}: ${l.value};`).join('\n');
    return `${HEADER_CSS}\n\n:root {\n${body}\n}\n`;
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

const sd = new StyleDictionary({
  source: [tokensPath],
  platforms: {
    css: {
      transformGroup: 'css',
      buildPath: resolve(repoRoot, 'web/src/styles') + '/',
      files: [{ destination: '_tokens.css', format: 'ownpulse/css-tokens' }],
    },
    swift: {
      transformGroup: 'js',
      buildPath: resolve(repoRoot, 'ios/OwnPulse/Theme') + '/',
      files: [{ destination: 'Tokens.swift', format: 'ownpulse/swift-tokens' }],
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
console.log('Design tokens built.');
