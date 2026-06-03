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

## Tests

The generator is covered by `web/tests/unit/design-tokens-generator.test.ts`
(name mapping, CSS↔Swift value parity, and an idempotency check that the build
reproduces the committed files byte-for-byte). Run with `npm test` in `web/`.

## Scope

This package only generates the palette/type/spacing/radii/shadow outputs. The
`chart.metric.*` tokens are intentionally omitted from `_tokens.css` — they are
consumed by the per-metric chart-color unit (B5). The strict CI drift gate
(fail if a rebuild leaves the tree dirty) is owned by a separate unit (B3); this
package's CI exposure is the web job, which installs and runs the generator via
the test above.
