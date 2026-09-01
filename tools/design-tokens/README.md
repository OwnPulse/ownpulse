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

This regenerates six committed outputs:

| Output | Consumer |
| ------ | -------- |
| `web/src/styles/_tokens.css` | imported by `web/src/styles/variables.css` for the light-mode palette, typography, spacing, radii, shadows |
| `web/src/components/explore/chartMetricColors.generated.ts` | per-metric chart colors + `INTERVENTION_COLOR` (re-exported from `web/src/components/explore/chartColors.ts`) + `PRIMARY_COLOR`/`ACCENT_COLOR` (imported directly by the analyze charts: `LagChart`, `ScatterChart`, `BeforeAfterChart`) |
| `web/src/components/dimensionColors.generated.ts` | `DIMENSION_COLORS` (the five check-in dimensions), consumed by `CheckinForm`, `ScoreRing`, `SparklineRow` |
| `ios/OwnPulse/Theme/Tokens.swift` | `OPColor` palette + `OPRadius` / `OPFontSize`, consumed by `ios/OwnPulse/Theme/Theme.swift` |
| `ios/OwnPulse/Theme/ChartColors.swift` | per-metric chart colors (iOS), shares its source of truth with `chartMetricColors.generated.ts` |
| `docs/design/tokens-generated.md` | human-readable reference |

The build is deterministic: running it on a clean tree yields no diff. The
generated files are committed so the apps build without running this tool.

## Contrast check (WCAG AA)

```bash
npm run check:contrast
```

`contrast.js` enumerates the text-on-surface and UI-component pairings the
light-mode palette implies, computes WCAG 2.1 relative-contrast ratios, and
asserts ≥4.5:1 for normal text and ≥3:1 for graphical objects / UI components.
It exits non-zero with a report of any failing pair + ratio. Covered:

- every text/feedback/interactive token on every `surface.*`;
- foreground tokens on the `primary.light` tint (chips, secondary-button hover,
  pending badges) and white text on the `primary` / `feedback.error` fills
  (filled buttons);
- a curated set of component pairings whose backdrop is a hand-written **rgba
  tint** rather than a token — the `.op-badge-success` / `.op-badge-error`
  tints, composited over each base surface (`componentPairings`).

Resting decorative borders (`color.border.*`) are reported informationally but
not asserted — they draw card outlines and dividers that WCAG 1.4.11 exempts as
not required to identify a component. Wired into the web CI job; run it before
editing colors.

If a pairing fails, fix the offending token VALUE in `docs/design/tokens.json`
(darken to the nearest compliant shade) and rerun `npm run build:tokens` so the
generated outputs stay in sync. Do not loosen the thresholds.

### Scope: tokens, not the whole UI

This is a **token** checker. A green `check:contrast` means the design-token
palette (and the curated token-fed component backdrops above) is AA-clean — it
does **not** mean the entire rendered UI is AA-clean. Components that hardcode
hex colors outside the token system are invisible to it by design; a grep-style
checker over every CSS literal would just duplicate component styles brittly.

Known out-of-scope offenders that currently fail AA (tracked for a follow-up
that tokenizes them):

- `web/src/components/dashboard/InsightCards.module.css` — `.tag_*` badges use
  `#fff` text on hardcoded fills (`#22c55e`, `#eab308`, `#f97316`, `#9ca3af`,
  `#3b82f6`), several well below 4.5:1.
- `web/src/components/dashboard/TodaysDoses.module.css` — `.greenCheck` /
  `.statusCompleted` use `#22c55e` text and `.statusSkipped` uses `#93c5fd`
  text on light surfaces, both failing.

The fix for these is to replace the literals with `feedback.*` (or new) tokens
so they fall under this checker; that component restyle is intentionally **not**
bundled into this tooling change.

## Tests

The generator is covered by `web/tests/unit/design-tokens-generator.test.ts`
(name mapping, CSS↔Swift value parity, `dimensionColors()`/`interventionColor()`/`brandColors()`
against the real token source — including the missing-token throw path — and
an idempotency check that the build reproduces all six committed files
byte-for-byte). The contrast math + palette compliance is covered by
`web/tests/unit/design-tokens-contrast.test.ts` (reference ratios, pairing
enumeration, and a regression guard that the committed palette passes every
asserted AA threshold, including the `color.dimension.*` and `chart.intervention`
graphical-object pairings). The `DIMENSION_COLORS` and `INTERVENTION_COLOR`
consumers are covered where they're used: `web/tests/unit/score-ring.test.tsx`,
`checkin-form.test.tsx`, `sparkline-row.test.tsx`, and `chart-colors.test.ts`
(which also asserts `PRIMARY_COLOR`/`ACCENT_COLOR` against the old pre-token
hexes and that the analyze charts import them rather than hardcoding a hex).
Run with `npm test` in `web/`.

## Scope

This package only generates the palette/type/spacing/radii/shadow outputs. The
`chart.metric.*` tokens are intentionally omitted from `_tokens.css` — they are
consumed by the per-metric chart-color unit (B5). The strict CI drift gate
(fail if a rebuild leaves the tree dirty) is owned by a separate unit (B3); this
package's CI exposure is the web job, which installs and runs the generator via
the test above.
