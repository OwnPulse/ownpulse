# OwnPulse Brand

## Name

**OwnPulse** — a compound of "Own" (sovereignty, possession) and "Pulse" (health signal, vital sign, rhythm). Reads as both "own your pulse" and "your own pulse."

## Tagline

> Your data. Your health. Your benefit.

## Values

1. **Data sovereignty** — you own it, you control it, you decide who sees it
2. **Non-judgmental** — all interventions are legitimate data; we track, we don't judge
3. **Open** — open source, open schema, open governance
4. **Cooperative** — collective benefit flows back to individual members

## Voice

Direct, technically credible, never condescending. We say "your data" not "your health journey." We say "export everything" not "we make it easy to take your data with you." We respect the reader's intelligence.

No marketing superlatives. No "revolutionary" or "game-changing." State what it does. Let the architecture speak.

## Design Tokens

The canonical source of truth for the palette, typography, spacing, radii, and shadows is [`tokens.json`](tokens.json) — a versioned [Design Tokens Community Group](https://design-tokens.github.io/community-group/format/) file. It is migrated verbatim from `web/src/styles/variables.css` and feeds the downstream Style Dictionary pipeline and the per-metric chart color slots.

This page describes the *intent* behind the brand system — the rationale, voice, and usage rules. When a concrete value (a hex code, a font stack, a type-scale step) is needed, treat `tokens.json` as authoritative. If a value on this page disagrees with `tokens.json`, `tokens.json` wins; fix this page.

## Color Palette

The palette avoids clinical blues, generic tech purples, and sterile whites. It draws from warm earth tones grounded by a deep neutral — trustworthy and human, not corporate or clinical.

All values live under the `color.*` group in [`tokens.json`](tokens.json). The headline roles:

- **Primary — warm terracotta:** `color.primary.default` (`#c2654a`), with `color.primary.hover` (`#9e4f38`) and `color.primary.light` (`#d4856e`). Grounded, human, distinctive.
- **Accent — muted teal:** `color.accent.default` (`#3d8b8b`), `color.accent.light` (`#5aadad`), `color.accent.dark` (`#2d6b6b`). Data, charts, interactive elements.
- **Neutral — warm charcoal:** `color.neutral.900` (`#1e1e1c`) through `color.neutral.50` (`#f7f7f4`). All text and UI chrome.
- **Surface:** `color.surface.bg` (`#fafaf7`) for the page base, `color.surface.elevated` (`#ffffff`) for cards, and `color.surface.bg-warm` (`#faf6f1`) for warm-tinted sections.
- **Text:** `color.text.default` (`#1e1e1c`), `color.text.secondary` (`#5e5e57`), `color.text.muted` (`#7a7a72`).
- **Border:** `color.border.default` (`#deded6`) for hairlines, `color.border.strong` (`#c2c2b9`) for emphasis.
- **Feedback — functional colors:** `color.feedback.success` (`#009e73`), `color.feedback.warning` (`#e69f00`), `color.feedback.error` (`#d55e00`), `color.feedback.error-light` (`#f5dede`). These come from the colorblind-safe Wong palette (see [Wong palette and colorblind safety](#wong-palette-and-colorblind-safety)), so success/warning/error stay distinguishable without relying on hue alone.
- **Dimension accents:** `color.dimension.gold` (`#c49a3c`), `color.dimension.sage` (`#5a8a5a`), `color.dimension.purple` (`#7b61c2`) — used to differentiate health "dimensions" in the UI.

### Usage

- **Primary (terracotta):** CTAs, links, brand accents, the logo mark
- **Accent (teal):** chart lines, data highlights, interactive elements, secondary buttons
- **Neutral scale:** all text and UI chrome — `neutral.900` for headings, `neutral.700` for body, `neutral.400` for secondary text. The dedicated `color.text.*` slots map onto this scale for text specifically.
- **Surface:** page backgrounds — `surface.bg` for the base, `surface.elevated` for cards

### Contrast and accessibility

The palette is built to meet **WCAG 2.1 level AA** contrast:

- **≥ 4.5:1** contrast ratio for normal-size text against its background.
- **≥ 3:1** for large text (≥ 24px, or ≥ 19px bold) and for graphical objects and UI components — chart lines, icons, focus rings, and borders that carry meaning.

The `color.text.*` and `color.neutral.*` ramps against the `color.surface.*` backgrounds are chosen to clear these thresholds. When pairing any foreground and background — including chart series against the surface — verify the ratio before shipping, and never rely on color alone to convey state. Pair it with text, icons, or shape.

## Typography

### Display: Source Serif 4

A variable-weight serif with sharp editorial character. Conveys authority and credibility without being stuffy. The serif form creates a distinctive contrast against the data-dense UI — health data platform, not another SaaS dashboard.

Used for: page titles, hero headlines, section headings (h1, h2).

### Body: IBM Plex Sans

A humanist sans-serif designed for long-form technical content. Slightly wider than typical UI fonts, with open counters that improve readability at small sizes. Chosen over Inter (ubiquitous, invisible) and Roboto (Google-branded). IBM Plex has a distinctive "engineered" quality that fits a data platform without feeling cold.

Used for: body text, UI labels, navigation, form inputs, code-adjacent content.

### Monospace: IBM Plex Mono

Paired with IBM Plex Sans for consistency. Used for data values, export format indicators, API references, and anywhere precision matters.

### Font Stack

The authoritative stacks live under `typography.font-family.*` in [`tokens.json`](tokens.json):

- `typography.font-family.display` — `"Source Serif 4", Georgia, "Times New Roman", serif`
- `typography.font-family.body` — `"IBM Plex Sans", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif`
- `typography.font-family.mono` — `"IBM Plex Mono", ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace`

### Type Scale

The type scale lives under `typography.font-size.*` in [`tokens.json`](tokens.json):

| Token | Value | Use |
|-------|-------|-----|
| `font-size.xs` | `0.75rem` (12px) | captions, fine print |
| `font-size.sm` | `0.875rem` (14px) | secondary text, labels |
| `font-size.base` | `1rem` (16px) | body text |
| `font-size.lg` | `1.125rem` (18px) | lead paragraphs |
| `font-size.xl` | `1.25rem` (20px) | h4, card titles |
| `font-size.2xl` | `1.5rem` (24px) | h3 |
| `font-size.3xl` | `2rem` (32px) | h2 |
| `font-size.4xl` | `2.75rem` (44px) | h1, hero |

## Spacing, Radii, and Shadows

These tokens live alongside color and typography in [`tokens.json`](tokens.json):

- **Spacing** (`spacing.*`): `content-padding` (`1.5rem`), `sidebar-width` (`220px`), `content-max-width` (`72rem`).
- **Radii** (`radii.*`): `sm` (`4px`), `md` (`8px`), `lg` (`12px`). Keep corners restrained — edges can be sharp; data is precise.
- **Shadows** (`shadow.*`): `sm` (`0 1px 2px rgba(0, 0, 0, 0.06)`), `md` (`0 4px 12px rgba(0, 0, 0, 0.08)`), `lg` (`0 8px 24px rgba(0, 0, 0, 0.12)`). Used sparingly to lift cards and overlays off the surface.

## Logo

The logo is the wordmark "OwnPulse" set in Source Serif 4 Bold, with "Own" in `color.neutral.900` and "Pulse" in `color.primary.default`. No icon/logomark for v1 — the wordmark is the identity. A small data-pulse motif (a simple SVG heartbeat/waveform line) may be added as a favicon and social card element.

## Imagery

No stock photography. No generic hero illustrations. Visual elements are:

- **Data visualization motifs** — SVG-based, abstract representations of health time-series. Grid lines, data points, subtle waveforms.
- **Structured whitespace** — generous margins, clear separation between sections. Let the content breathe.
- **The grid** — underlying 12-column grid visible in card layouts and data tables. Precision as aesthetic.

## Wong palette and colorblind safety

Charts plot many series at once, so color must carry meaning for everyone — including the roughly 1 in 12 men and 1 in 200 women with some form of color vision deficiency. The per-metric chart colors under `chart.metric.*` in [`tokens.json`](tokens.json) draw on the [Wong colorblind-safe palette](https://www.nature.com/articles/nmeth.1618) (Bang Wong, *Nature Methods*, 2011), a set of eight hues chosen to stay distinguishable under deuteranopia, protanopia, and tritanopia.

Each metric gets a fixed, accessible hue so the same signal reads the same way across the app:

| Metric token | Value | Notes |
|--------------|-------|-------|
| `chart.metric.heart_rate` | `#d55e00` (vermillion) | reuses an existing palette value |
| `chart.metric.hrv` | `#009e73` (bluish green) | reuses an existing palette value |
| `chart.metric.sleep_duration` | `#7b61c2` (purple) | distinct from the green/orange metrics |
| `chart.metric.weight` | `#c49a3c` (gold) | reuses an existing palette value |
| `chart.metric.glucose` | `#0072b2` (blue) | Wong blue, far from the warm brand hues |
| `chart.metric.bp_systolic` | `#cc79a7` (reddish purple) | deliberately avoids the brand-primary `#c2654a` so series don't collide with brand-colored chrome |
| `chart.metric.bp_diastolic` | `#56b4e9` (sky blue) | pairs with systolic while staying separable |

When a metric has no assigned slot, charts cycle through `chart.metric.fallback` — a twelve-color sequence (also rooted in Wong hues) ordered to maximise adjacent-color separation.

Two rules keep this robust:

- **Never rely on hue alone.** Pair color with direct labels, distinct line styles, markers, or position so a chart still reads in grayscale.
- **Keep the assignments stable.** A metric's color is part of its identity; changing it breaks the reader's learned association. Change `tokens.json`, never a one-off override.

### Trend indicators

Trend direction (a metric going up vs. down vs. holding steady) follows the same "never rely on hue alone" rule. Direction is always carried by a **directional arrow** first, with color as secondary reinforcement:

| Direction | Arrow (SF Symbol) | Color |
|-----------|-------------------|-------|
| Up | `arrow.up.right` | `#d55e00` Wong vermillion (the `chart.metric.heart_rate` token) |
| Down | `arrow.down.right` | `#0072b2` Wong blue (the `chart.metric.glucose` token) |
| Flat | `arrow.forward` | neutral secondary |

The arrow shape alone distinguishes the three states in grayscale and under any color vision deficiency, so the indicator never depends on telling red from green. The earlier red(up)/green(down) scheme is intentionally gone: it failed red-green color vision and was also semantically inverted. On iOS this mapping lives in a single `TrendDirection` type so the cards cannot drift apart.

## What We Don't Do

- No purple gradients
- No generic "connected health" illustrations (people with smartwatches, hearts with WiFi symbols)
- No stock photos of any kind
- No Inter, Roboto, or system-default typography
- No rounded-everything soft UI — edges can be sharp, data is precise
- No dark mode as primary (light mode first, dark mode as preference)
- No emojis in product UI (acceptable in docs and community spaces)
