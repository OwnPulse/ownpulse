<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# design-tokens

Style Dictionary pipeline that turns the canonical token source into the
per-platform files the apps consume.

## Source of truth

[`docs/design/tokens.json`](../../docs/design/tokens.json) — edit this, never
the generated outputs.

## Build

```bash
npm install
npm run build:tokens
```

This regenerates three committed outputs:

| Output | Consumer |
| ------ | -------- |
| `web/src/styles/_tokens.css` | imported by `web/src/styles/variables.css` for the light-mode palette, typography, spacing, radii, shadows |
| `ios/OwnPulse/Theme/Tokens.swift` | `OPColor` palette + `OPRadius` / `OPFontSize`, consumed by `ios/OwnPulse/Theme/Theme.swift` |
| `docs/design/tokens-generated.md` | human-readable reference |

The build is deterministic: running it on a clean tree yields no diff. The
generated files are committed so the apps build without running this tool.

## Contrast check (WCAG AA)

```bash
npm run check:contrast
```

`contrast.js` enumerates every text-on-surface and active UI-component pairing
the light-mode palette implies, computes WCAG 2.1 relative-contrast ratios, and
asserts ≥4.5:1 for normal text and ≥3:1 for graphical objects / UI components.
It exits non-zero with a report of any failing pair + ratio. Resting decorative
borders (`color.border.*`) are reported informationally but not asserted — they
draw card outlines and dividers that WCAG 1.4.11 exempts as not required to
identify a component. Wired into the web CI job; run it before editing colors.

If a pairing fails, fix the offending token VALUE in `docs/design/tokens.json`
(darken to the nearest compliant shade) and rerun `npm run build:tokens` so the
generated outputs stay in sync. Do not loosen the thresholds.

## Tests

The generator is covered by `web/tests/unit/design-tokens-generator.test.ts`
(name mapping, CSS↔Swift value parity, and an idempotency check that the build
reproduces the committed files byte-for-byte). The contrast math + palette
compliance is covered by `web/tests/unit/design-tokens-contrast.test.ts`
(reference ratios, pairing enumeration, and a regression guard that the
committed palette passes every asserted AA threshold). Run with `npm test` in
`web/`.

## Scope

This package only generates the palette/type/spacing/radii/shadow outputs. The
`chart.metric.*` tokens are intentionally omitted from `_tokens.css` — they are
consumed by the per-metric chart-color unit (B5). The strict CI drift gate
(fail if a rebuild leaves the tree dirty) is owned by a separate unit (B3); this
package's CI exposure is the web job, which installs and runs the generator via
the test above.
