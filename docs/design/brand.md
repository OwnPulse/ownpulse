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

## Color Palette

The palette avoids clinical blues, generic tech purples, and sterile whites. It draws from warm earth tones grounded by a deep neutral — trustworthy and human, not corporate or clinical.

```css
:root {
  /* Primary — warm terracotta: grounded, human, distinctive */
  --color-primary: #c2654a;
  --color-primary-light: #d4856e;
  --color-primary-dark: #9e4f38;

  /* Neutral — warm charcoal: text, headings, UI chrome */
  --color-neutral-900: #1e1e1c;
  --color-neutral-800: #2d2d2a;
  --color-neutral-700: #44443f;
  --color-neutral-600: #5e5e57;
  --color-neutral-500: #7a7a72;
  --color-neutral-400: #9c9c93;
  --color-neutral-300: #c2c2b9;
  --color-neutral-200: #deded6;
  --color-neutral-100: #eeeeea;
  --color-neutral-50: #f7f7f4;

  /* Accent — muted teal: data, charts, interactive elements */
  --color-accent: #3d8b8b;
  --color-accent-light: #5aadad;
  --color-accent-dark: #2d6b6b;

  /* Signal — functional colors */
  --color-success: #5a8a5a;
  --color-warning: #c49a3c;
  --color-error: #b54a4a;

  /* Surface — page backgrounds */
  --color-surface: #fafaf7;
  --color-surface-elevated: #ffffff;
}
```

### Usage

- **Primary (terracotta):** CTAs, links, brand accents, the logo mark
- **Accent (teal):** chart lines, data highlights, interactive elements, secondary buttons
- **Neutral scale:** all text and UI chrome — 900 for headings, 700 for body, 400 for secondary text
- **Surface:** page backgrounds — `surface` for the base, `surface-elevated` for cards

## Typography

### Display: Source Serif 4

A variable-weight serif with sharp editorial character. Conveys authority and credibility without being stuffy. The serif form creates a distinctive contrast against the data-dense UI — health data platform, not another SaaS dashboard.

Used for: page titles, hero headlines, section headings (h1, h2).

### Body: IBM Plex Sans

A humanist sans-serif designed for long-form technical content. Slightly wider than typical UI fonts, with open counters that improve readability at small sizes. Chosen over Inter (ubiquitous, invisible) and Roboto (Google-branded). IBM Plex has a distinctive "engineered" quality that fits a data platform without feeling cold.

Used for: body text, UI labels, navigation, form inputs, code-adjacent content.

### Monospace: IBM Plex Mono

Paired with IBM Plex Sans for consistency. Used for data values, export format indicators, API references, and anywhere precision matters.

### Font Stack (CSS)

```css
:root {
  --font-display: 'Source Serif 4', 'Source Serif Pro', Georgia, 'Times New Roman', serif;
  --font-body: 'IBM Plex Sans', 'Helvetica Neue', Arial, sans-serif;
  --font-mono: 'IBM Plex Mono', 'SF Mono', 'Fira Code', monospace;
}
```

### Type Scale

```css
:root {
  --text-xs: 0.75rem;    /* 12px — captions, fine print */
  --text-sm: 0.875rem;   /* 14px — secondary text, labels */
  --text-base: 1rem;     /* 16px — body text */
  --text-lg: 1.125rem;   /* 18px — lead paragraphs */
  --text-xl: 1.25rem;    /* 20px — h4, card titles */
  --text-2xl: 1.5rem;    /* 24px — h3 */
  --text-3xl: 2rem;      /* 32px — h2 */
  --text-4xl: 2.75rem;   /* 44px — h1, hero */
  --text-5xl: 3.5rem;    /* 56px — display, landing hero */
}
```

## Logo

The logo is the wordmark "OwnPulse" set in Source Serif 4 Bold, with "Own" in `--color-neutral-900` and "Pulse" in `--color-primary`. No icon/logomark for v1 — the wordmark is the identity. A small data-pulse motif (a simple SVG heartbeat/waveform line) may be added as a favicon and social card element.

## Imagery

No stock photography. No generic hero illustrations. Visual elements are:

- **Data visualization motifs** — SVG-based, abstract representations of health time-series. Grid lines, data points, subtle waveforms.
- **Structured whitespace** — generous margins, clear separation between sections. Let the content breathe.
- **The grid** — underlying 12-column grid visible in card layouts and data tables. Precision as aesthetic.

## What We Don't Do

- No purple gradients
- No generic "connected health" illustrations (people with smartwatches, hearts with WiFi symbols)
- No stock photos of any kind
- No Inter, Roboto, or system-default typography
- No rounded-everything soft UI — edges can be sharp, data is precise
- No dark mode as primary (light mode first, dark mode as preference)
- No emojis in product UI (acceptable in docs and community spaces)
