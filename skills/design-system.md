# Bisque Design System

A typography-first design system for native GPU-rendered interfaces. No boxes, no cards, no fills. Structure is communicated through type scale, opacity, whitespace, and thin rules.

Informed by Tufte, Bringhurst, Tschichold, Muller-Brockmann, Wilkinson, Bostock, Bremer, Ortiz, Ware, and Butterick. Full research in `skills/sources/`.

---

## 1. Foundations

### Background

Bisque `#FFE4C4` / `rgb(255, 228, 196)` / `[1.0, 0.894, 0.769, 1.0]`

23% blue light reduction vs white. Contrast with black: 17.3:1 (exceeds WCAG AAA). Warm parchment tone reduces eye strain during sustained reading.

### Ink

A single hue — black — at six opacity levels. No other structural colors. Chromatic color is reserved exclusively for data encoding and stateful elements.

| Token | Opacity | RGBA | Use |
|---|---|---|---|
| `ink.primary` | 100% | `[0, 0, 0, 1.0]` | Headings, primary content |
| `ink.section` | 80% | `[0, 0, 0, 0.80]` | Section titles, strong labels |
| `ink.body` | 70% | `[0, 0, 0, 0.70]` | Body text (passes AA at all sizes on bisque) |
| `ink.secondary` | 50% | `[0, 0, 0, 0.50]` | Descriptions, supplementary text |
| `ink.annotation` | 40% | `[0, 0, 0, 0.40]` | Timestamps, metadata, captions |
| `ink.rule` | 15% | `[0, 0, 0, 0.15]` | Hairline rules, dividers |
| `ink.ghost` | 8% | `[0, 0, 0, 0.08]` | Disabled, placeholder |

**Contrast verification:** 50% black on bisque composites to ~3.85:1 (passes AA for large text >= 18pt only). For normal body text, use >= 60% opacity minimum.

### Accent colors (when needed)

Reserve chromatic color for actionable or stateful elements only: links, active selections, errors, voice indicators. When accent colors are used, they should share a consistent CIELAB L* value (Solarized principle) so they carry equal visual weight.

---

## 2. Typography

### Fonts

| Role | Family | Fallback stack | Notes |
|---|---|---|---|
| Readable text | Optima | Palatino, Georgia, serif | Humanist sans with calligraphic roots |
| Code/data | Monaco | Menlo, Cascadia Code, monospace | Clear glyph distinction (0/O, 1/l/I) |

Two families. No more. Hierarchy through size, weight, and opacity — not through additional typefaces.

### Type scale

**Perfect Fourth (1.333)** ratio with **18px base**. The Perfect Fourth provides clear hierarchy without the dramatic jumps of wider scales, appropriate for a dashboard/tool interface.

| Step | Token | Size (px) | Use |
|---|---|---|---|
| -2 | `type.xs` | 10.1 | Fine print, legal |
| -1 | `type.sm` | 13.5 | Captions, timestamps |
| 0 | `type.base` | 18.0 | Body text |
| +1 | `type.lg` | 24.0 | Lead text, data values |
| +2 | `type.xl` | 32.0 | Section headings |
| +3 | `type.2xl` | 42.6 | Page titles |
| +4 | `type.3xl` | 56.8 | Display, hero |
| +5 | `type.4xl` | 75.7 | Splash text |

Formula: `size = 18.0 * 1.333^step`

### Line height

| Context | Ratio | With 18px base |
|---|---|---|
| Body text | 1.5 | 27px → snap to 28px (baseline) |
| Headings (24-42px) | 1.2 | Computed per heading |
| Display (56px+) | 1.05-1.1 | Tight |
| Captions | 1.4 | |

### Baseline grid

**Unit: 28px** (18px body × 1.5 line-height, rounded to nearest multiple of 4 for clean pixel math at 2x).

All vertical spacing is a multiple of this unit:

| Token | Multiple | Value |
|---|---|---|
| `space.quarter` | 0.25× | 7px |
| `space.half` | 0.5× | 14px |
| `space.one` | 1× | 28px |
| `space.two` | 2× | 56px |
| `space.three` | 3× | 84px |
| `space.four` | 4× | 112px |

### Letter spacing

| Context | Tracking |
|---|---|
| Body text | 0 (font default) |
| ALL CAPS labels | +0.08em |
| Display (56px+) | -0.01em |
| Small annotations | +0.02em |

### Measure (line length)

Target: **66 characters** (~33em). Acceptable: 45-75. For multi-column: 40-50.

At 18px Optima, this is approximately **580px** max content width for single-column body text.

---

## 3. Layout

### Margins

Adapted from the Van de Graaf / Tschichold canon for a single-screen context:

| Edge | Fraction | At 1280px wide | At 800px tall |
|---|---|---|---|
| Left | 1/9 | 142px | — |
| Top | 1/9 | — | 89px |
| Right | 2/9 | 284px | — |
| Bottom | 2/9 | — | 178px |

For symmetric layouts (dashboard panels), use the average: `1.5/9 = 1/6` of width for left and right margins.

**Practical minimum:** 48px left margin. This provides enough space for a strong left text edge.

### Grid

12-column grid with 24px gutters. Column widths compute from:

```
column_width = (content_width - 11 * 24) / 12
```

### Section structure

Sections are delineated by:
1. A section title in `type.xl` at `ink.section` (80% opacity)
2. A 0.5px horizontal rule in `ink.rule` (15% opacity) directly below the title baseline
3. Content below, separated from the rule by `space.half` (14px)

Between sections: `space.two` (56px).

### Headings proximity rule

Headings have **1.5× more space above than below**, so they visually bind to the content they introduce:

- Heading top margin: `space.two` (56px)
- Heading bottom margin: `space.half` (14px)
- Rule immediately follows heading baseline

### No containers

There are no:
- Boxes or cards with background fills
- Rounded rectangles
- Panels with borders
- Progress bars (use text percentages)
- Colored badges or pills (use bracketed text: `[error]`, `[active]`)
- 3D effects, shadows, or gradients

Grouping is achieved through **proximity** (Gestalt) and **alignment** (continuity). Items within a group are separated by `space.half`. Groups are separated by `space.one` or more.

---

## 4. Principles (The Constraints)

These are non-negotiable. When in doubt, apply the constraint.

### P1. Every pixel justifies its existence
If a visual element does not encode data or structural grammar, remove it. (Tufte: maximize data-ink ratio)

### P2. Typography is the primary information channel
Size, weight, opacity, tracking, and typeface switching communicate structure. No visual chrome needed. (Tschichold, Bringhurst)

### P3. Whitespace is active, not empty
Spacing creates hierarchy: larger gaps = larger conceptual separations. The ratio between within-group and between-group spacing must be at least 2:1. (Gestalt proximity, Muller-Brockmann)

### P4. Color encodes data, never decorates
Chromatic color appears only when it carries meaning (status, selection, error). Structural elements use the black opacity ramp. When color does appear, it pops out pre-attentively because the surrounding field is monochrome. (Tufte, Ware)

### P5. Strong left edge
All text aligns to a consistent left margin. The eye follows the F-pattern; a ragged left edge destroys scannability. (NN/g, Muller-Brockmann)

### P6. Show the data, not the chrome
Default to no borders, no backgrounds, no decorative elements. Thin rules (0.5px, 15% opacity) are the maximum permitted structural decoration. (Tufte: "above all else, show the data")

### P7. Beauty earns attention
Visual appeal is functional, not ornamental. Well-set Optima on bisque with generous whitespace is the condition under which sustained attention becomes possible. (Bremer, Tufte)

### P8. Speed is a design property
All supplemental information in under 100ms. Rendering at 60fps. Latency is not a performance metric — it is a design parameter. (fullyparsed.com, Bostock)

### P9. Density without clutter
Pack information densely, but use layering, separation, and typographic hierarchy to prevent clutter. "There is no such thing as information overload. There is only bad design." (Tufte)

### P10. Animation encodes information
Never animate for decoration. Transitions must communicate what changed, what was added, what was removed. Object constancy: preserve the identity of data elements across state changes. (Bostock)

---

## 5. Applying the System

### Rendering a section

```
[space.two gap from previous section]

SECTION TITLE                        ← type.xl, ink.section, ALL CAPS, +0.08em tracking
────────────────────────────────     ← 0.5px rule, ink.rule

[space.half gap]

Label           Value                ← type.base, ink.body / ink.primary
Label           Value
Label           Value

[space.two gap to next section]
```

### Rendering a data value

```
42.7%                                ← type.lg, ink.primary, Monaco
cpu usage                            ← type.sm, ink.annotation, Optima, ALL CAPS, +0.08em
```

### Rendering a status

```
Recording [active]                   ← type.base, ink.body. "[active]" in ink.annotation
```

### Rendering a heading

```
[space.two gap]

Page Title                           ← type.2xl, ink.primary, Optima
[space.half gap]

Body text begins here...             ← type.base, ink.body
```

---

## 6. Implementation

The design system is implemented as a Rust module at `src/design.rs`. It provides:

- `Token` constants for all colors, sizes, and spacing values
- `type_size(step)` function for computing scale sizes
- `snap_to_baseline(y)` for vertical rhythm enforcement
- `column_width(container, cols, gap)` for grid computation
- `contrast_ratio(fg, bg)` for WCAG verification
- `lerp(a, b, t)` and basic easing for animation

No external crate dependencies beyond what the project already uses (vello, peniko, skrifa, parley, kurbo).
