# Rust Crates for a Typography-First Design System

Research for bisque-computer (vello/winit native macOS app).

**Current stack:** vello 0.7, winit 0.30, skrifa 0.40, peniko 0.6, parley 0.7

---

## 1. Color Manipulation

### Already in the Stack: `color` (via peniko)

Peniko 0.6 depends on the linebender [`color`](https://crates.io/crates/color) crate (v0.3.x), which already provides:

- Static color types: `OpaqueColor`, `AlphaColor`, `PremulColor`
- Runtime color types: `DynamicColor`
- Color space conversions via `.convert()` method
- CSS Color Level 4 compliance (sRGB, Display P3, linear sRGB, Oklab, Oklch, Lab, LCH, and more)
- `f32` component values, linear sRGB as central hub
- ACEScg color space (added in 0.2.0)
- License: MIT/Apache-2.0

**Verdict:** This is already a transitive dependency. For sRGB/Oklab/Lab conversions, it should be the first choice. No new dep needed for basic conversions.

- Docs: https://docs.rs/color/latest/color/
- GitHub: https://github.com/linebender/color

### `palette`

- **URL:** https://crates.io/crates/palette
- **Version:** 0.7.6
- **License:** MIT/Apache-2.0
- **What it provides:** The most comprehensive color crate in the Rust ecosystem. Covers sRGB, HSL, HSV, HWB, Lab, LCH, XYZ, xyY, Oklab, Oklch. Color operations as traits (lighten/darken, hue shift, saturate/desaturate, mix/interpolate, SVG blend). `#[no_std]` support. Optional serde/rand/bytemuck integration. Generic over color space and float type.
- **Size/deps:** Moderate — pulls in several sub-crates (`palette_derive`, etc.). Heavier than the linebender `color` crate.
- **Maintenance:** Actively maintained (Ogeon). Last release April 2024.
- **When to use:** If you need HSL/HSV manipulation, lighten/darken operations, complementary color computation, palette generation, or operations not in the `color` crate. The `color` crate covers conversion but palette covers *manipulation*.
- **Recommendation:** Worth adding if you need color manipulation operations beyond simple conversion. The trait-based design is clean.

### `colorsys`

- **URL:** https://crates.io/crates/colorsys
- **License:** MIT
- **What it provides:** RGB(a), HSL(a), CMYK conversion and mutation. CSS string parsing (`to_css_string()`, `to_hex_string()`). `f64` values. `#[no_std]` support.
- **Size/deps:** Small (~29K downloads/month).
- **Maintenance:** Less active than palette.
- **Recommendation:** Skip. `palette` is strictly more capable. If you only need HSL, `color` may already suffice.

### `oklab`

- **URL:** https://crates.io/crates/oklab
- **Version:** 1.1.0
- **License:** MIT
- **What it provides:** Lightweight Oklab color space conversion and blending. Very small and focused.
- **Size/deps:** Tiny (~7KB). Zero dependencies.
- **Recommendation:** Skip. The linebender `color` crate already provides Oklab. Only useful if you need a standalone, zero-dep Oklab without the rest of the color stack.

### `csscolorparser`

- **URL:** https://crates.io/crates/csscolorparser
- **Version:** 0.7.2
- **License:** MIT/Apache-2.0
- **What it provides:** Parse CSS color strings per W3C CSS Color Module Level 4. Supports named colors, hex, rgb(), hsl(), hwb(), lab(), lch(), oklab(), oklch().
- **Size/deps:** 38.8 KiB. ~3.3M total downloads. Well-used.
- **Maintenance:** Actively maintained.
- **Recommendation:** Useful if you want to let users specify colors in CSS syntax (e.g., in a config file or theme DSL). Otherwise skip — the `color` crate already handles color spaces.

### WCAG Contrast Ratio

- **`contrast-checker`** (https://crates.io/crates/contrast-checker) — CLI tool for WCAG 2.0 contrast ratio. Not really a library.
- No well-maintained Rust crate specifically for WCAG contrast ratio computation as a library dependency.

**Implement yourself.** The formula is trivial:

```rust
/// Relative luminance per WCAG 2.0
fn relative_luminance(r: f32, g: f32, b: f32) -> f32 {
    fn linearize(c: f32) -> f32 {
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    }
    0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

/// WCAG 2.0 contrast ratio (returns value in 1.0..=21.0)
fn contrast_ratio(lum1: f32, lum2: f32) -> f32 {
    let (lighter, darker) = if lum1 > lum2 { (lum1, lum2) } else { (lum2, lum1) };
    (lighter + 0.05) / (darker + 0.05)
}
// AA normal text: >= 4.5:1, AA large text: >= 3.0:1
// AAA normal text: >= 7.0:1, AAA large text: >= 4.5:1
```

### Palette Generation

No dedicated Rust crate for generating palettes from a base color is mature enough to recommend. Implement yourself using the `palette` or `color` crate:

```rust
// Complementary: rotate hue by 180 degrees in Oklch
// Analogous: rotate hue by +/- 30 degrees
// Triadic: rotate hue by +/- 120 degrees
// Tints: increase lightness
// Shades: decrease lightness
```

---

## 2. Layout and Spacing

### `taffy`

- **URL:** https://crates.io/crates/taffy
- **Version:** 0.7.6
- **License:** MIT
- **What it provides:** High-performance CSS Block, Flexbox, and CSS Grid layout engine. Two APIs: high-level `TaffyTree` and low-level trait-based. Supports `calc()` values. Used by Dioxus, Zed editor (fork), and others.
- **Size/deps:** Moderate. `#[no_std]` compatible with alloc.
- **Maintenance:** Very actively maintained by DioxusLabs. Major releases regularly.
- **Recommendation:** **Strong candidate** if you need a real layout engine for the dashboard. Could replace hand-rolled layout math. The CSS Grid support is particularly useful for dashboard panel layouts. Integrating with vello's coordinate system would require a mapping layer but is straightforward.

### `morphorm`

- **URL:** https://crates.io/crates/morphorm
- **GitHub:** https://github.com/vizia/morphorm
- **License:** MIT
- **What it provides:** UI layout engine from the Vizia project. Simpler model than taffy — not CSS-based but instead uses a custom property-based system. Determines size and position of nodes in a tree.
- **Maintenance:** Actively maintained (part of Vizia ecosystem).
- **Recommendation:** Consider if taffy's CSS model is overkill. Morphorm is simpler but less standard. Taffy is the safer bet due to wider adoption.

### Modular Type Scale, Grid Computation, Baseline Grid

**Implement yourself.** These are pure arithmetic:

```rust
/// Modular type scale: base * ratio^step
fn type_scale(base: f32, ratio: f32, step: i32) -> f32 {
    base * ratio.powi(step)
}

// Common ratios:
// Minor second: 1.067    Major second: 1.125
// Minor third:  1.200    Major third:  1.250
// Perfect fourth: 1.333  Augmented fourth: 1.414
// Perfect fifth:  1.500  Golden ratio: 1.618

/// Snap a value to the nearest baseline grid line
fn snap_to_baseline(y: f32, baseline_height: f32) -> f32 {
    (y / baseline_height).round() * baseline_height
}

/// N-column grid: compute column width given container width, columns, and gap
fn column_width(container_w: f32, columns: u32, gap: f32) -> f32 {
    (container_w - gap * (columns - 1) as f32) / columns as f32
}
```

No crate needed. These are 1-3 line functions.

---

## 3. Typography Utilities

### Already in the Stack

**`skrifa` 0.40** (https://crates.io/crates/skrifa) provides:
- `FontRef` for zero-copy font access
- `charmap()` for character-to-glyph mapping
- `glyph_metrics()` for per-glyph advance widths and bounds
- `Metrics` struct: units_per_em, ascent, descent, leading, underline/strikeout position and size
- Variable font axis enumeration and named instances
- OpenType table access

**`parley` 0.7** (https://crates.io/crates/parley) provides:
- Rich text layout (x/y coordinates for each glyph)
- Line breaking, bidi reordering, alignment
- Re-linebreaking/re-aligning when wrap width changes
- Text selection and editing utilities
- Depends on HarfRust (text shaping), Fontique (font enumeration/fallback), ICU4X (i18n)

**Together, skrifa + parley cover virtually all typography needs for a vello app.** Parley handles layout; skrifa handles font metrics.

### `swash`

- **URL:** https://crates.io/crates/swash
- **Version:** 0.2.6
- **License:** MIT/Apache-2.0
- **What it provides:** Font introspection, complex text shaping, glyph rendering. Apple Advanced Typography support (morx, kerx). Variable font support. Synthesized vertical metrics. Claims 10-20% faster than FreeType/HarfBuzz in microbenchmarks.
- **Recommendation:** **Skip.** Parley + skrifa (via HarfRust) already cover shaping and metrics. Swash was the predecessor approach; the linebender stack has moved to parley/skrifa/fontique. Adding swash would create a parallel font pipeline.

### Typographic Scale Computation

**Implement yourself.** See the `type_scale()` function above. A type scale is just `base * ratio.pow(step)`.

### Text Measurement

Parley already provides this. After building a `Layout`, you get line heights, widths, and glyph positions. For single-glyph measurement, use skrifa's `glyph_metrics().advance_width(glyph_id)`.

---

## 4. Math / Proportion / Animation Utilities

### Already in the Stack: `kurbo`

Kurbo (https://crates.io/crates/kurbo, v0.12) is a transitive dependency via vello. It provides:
- Bezier curves (quadratic, cubic), arcs, ellipses
- Rectangles, rounded rectangles, circles, lines
- Path operations, area computation, bounding boxes
- High-accuracy curve algorithms

No need to add lyon or similar for basic curve math.

### `easer`

- **URL:** https://crates.io/crates/easer
- **Version:** 0.3.0
- **License:** MIT
- **What it provides:** Robert Penner's easing functions (ease-in, ease-out, ease-in-out for quad, cubic, quart, quint, sine, expo, circ, elastic, back, bounce). Tiny: 7 KiB, zero meaningful deps.
- **Downloads:** ~909K total.
- **Recommendation:** **Good lightweight option** for animation easing if you plan to animate UI transitions. However, also easy to implement yourself if you only need a few.

### `keyframe`

- **URL:** https://crates.io/crates/keyframe
- **What it provides:** Animation keyframes with easing functions, user-defined Bezier curves (CSS cubic-bezier style), keyframable curves. `ease(function, from, to, time)` API.
- **Recommendation:** Useful if you want keyframe-based animation. Heavier than `easer`. Skip unless you need keyframe sequences.

### `enterpolation`

- **URL:** https://crates.io/crates/enterpolation
- **What it provides:** Linear interpolation, Bezier curves, B-splines, NURBS, extrapolation. Generic and composable.
- **Recommendation:** Overkill for UI animation. Better suited for scientific/CAD use. Skip.

### Golden Ratio, Modular Scale

**Implement yourself:**

```rust
const PHI: f32 = 1.618_033_988_75;
const SQRT2: f32 = 1.414_213_562_37;

fn modular_scale(base: f32, ratio: f32, step: i32) -> f32 {
    base * ratio.powi(step)
}

// Linear interpolation
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// Smoothstep (cubic Hermite)
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
```

### Easing Functions (DIY alternative to `easer`)

If you only need a few easing curves:

```rust
fn ease_in_quad(t: f32) -> f32 { t * t }
fn ease_out_quad(t: f32) -> f32 { t * (2.0 - t) }
fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 { 2.0 * t * t } else { -1.0 + (4.0 - 2.0 * t) * t }
}
fn ease_out_cubic(t: f32) -> f32 { let t1 = t - 1.0; t1 * t1 * t1 + 1.0 }
fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t } else { (t - 1.0) * (2.0 * t - 2.0) * (2.0 * t - 2.0) + 1.0 }
}
```

---

## 5. Design Token / Style Systems

### No Mature Standalone Crate

The Rust ecosystem does not have a mature, framework-agnostic design token crate. What exists:

- **`iced_plus_tokens`** — Tailwind/Chakra-inspired tokens, but tightly coupled to the Iced framework.
- **`theme`** — Flexible theming for WASM frameworks (Yew, Leptos, Dioxus). Not useful for vello.

### Recommendation: Build Your Own

For a vello/winit app, a design token system is best expressed as a Rust module with const/static values:

```rust
pub mod tokens {
    use peniko::Color;

    // Color tokens
    pub const BG_PRIMARY: Color = Color::from_rgba8(255, 228, 196, 255);  // bisque
    pub const TEXT_PRIMARY: Color = Color::from_rgba8(0, 0, 0, 255);

    // Typography tokens
    pub const FONT_FAMILY_BODY: &str = "Optima";
    pub const FONT_FAMILY_MONO: &str = "Monaco";
    pub const TYPE_BASE: f32 = 16.0;
    pub const TYPE_RATIO: f32 = 1.250;  // major third

    pub fn type_size(step: i32) -> f32 {
        TYPE_BASE * TYPE_RATIO.powi(step)
    }

    // Spacing tokens (based on baseline grid)
    pub const BASELINE: f32 = 8.0;
    pub fn space(n: u32) -> f32 { BASELINE * n as f32 }

    // Elevation / layering
    pub const BORDER_RADIUS_SM: f32 = 4.0;
    pub const BORDER_RADIUS_MD: f32 = 8.0;
}
```

This is type-safe, zero-cost, and perfectly suited to a compiled native app. No crate needed.

---

## Summary: Recommended Additions

| Crate | Purpose | Priority | Size Impact |
|-------|---------|----------|-------------|
| **`palette`** | Color manipulation (HSL/HSV ops, lighten/darken, mix) | Medium | Moderate |
| **`taffy`** | CSS Flexbox/Grid layout engine | High (if complex layout needed) | Moderate |
| **`easer`** | Easing functions for animation | Low | Tiny (7 KiB) |
| **`csscolorparser`** | Parse CSS color strings from config/theme files | Low | Small (39 KiB) |

### Already Covered by Current Dependencies

| Need | Covered By |
|------|-----------|
| Color space conversion (sRGB, Oklab, Lab, Oklch) | `color` (via peniko) |
| Font metrics (ascent, descent, advance, UPM) | `skrifa` |
| Text layout, line breaking, shaping | `parley` (via HarfRust, ICU4X) |
| Font enumeration and fallback | `fontique` (via parley) |
| Bezier curves, shapes, path math | `kurbo` (via vello) |

### Implement Yourself (No Crate Needed)

| Need | Complexity |
|------|-----------|
| WCAG contrast ratio | ~15 lines |
| Modular type scale | 1 line: `base * ratio.powi(step)` |
| Baseline grid snapping | 1 line: `(y / grid).round() * grid` |
| Column grid computation | 1 line |
| Golden ratio / proportions | Constants |
| Palette generation (complementary, analogous, triadic) | ~20 lines with Oklch hue rotation |
| Design tokens | A Rust module with `const` values |
| Basic easing functions (quad, cubic) | ~10 lines each |
| Linear interpolation / lerp | 1 line |

---

## Sources

- [palette crate](https://crates.io/crates/palette) | [GitHub](https://github.com/Ogeon/palette) | [Docs](https://docs.rs/palette/latest/palette/)
- [oklab crate](https://crates.io/crates/oklab)
- [colorsys crate](https://crates.io/crates/colorsys) | [GitHub](https://github.com/emgyrz/colorsys.rs)
- [csscolorparser crate](https://crates.io/crates/csscolorparser)
- [color crate (linebender)](https://crates.io/crates/color) | [GitHub](https://github.com/linebender/color) | [Docs](https://docs.rs/color/latest/color/)
- [taffy crate](https://crates.io/crates/taffy) | [GitHub](https://github.com/DioxusLabs/taffy)
- [morphorm crate](https://crates.io/crates/morphorm) | [GitHub](https://github.com/vizia/morphorm)
- [parley crate](https://crates.io/crates/parley) | [GitHub](https://github.com/linebender/parley)
- [skrifa crate](https://crates.io/crates/skrifa)
- [swash crate](https://crates.io/crates/swash) | [GitHub](https://github.com/dfrg/swash)
- [kurbo crate](https://crates.io/crates/kurbo) | [GitHub](https://github.com/linebender/kurbo)
- [easer crate](https://crates.io/crates/easer) | [GitHub](https://github.com/orhanbalci/rust-easing)
- [keyframe crate](https://crates.io/crates/keyframe)
- [enterpolation crate](https://crates.io/crates/enterpolation)
- [peniko crate](https://crates.io/crates/peniko) | [Docs](https://docs.rs/peniko/latest/peniko/)
- [contrast-checker crate](https://crates.io/crates/contrast-checker)
- [Linebender color blog post](https://linebender.org/blog/tmil-11/)
