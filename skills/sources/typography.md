# Foundational Principles of Typography for Screen and Print

A reference document covering type scale systems, vertical rhythm, font pairing, typographic hierarchy, measure, and microtypography. All numeric values and ratios are intended to be directly implementable in code.

---

## 1. Type Scale Systems

A **modular scale** is a sequence of font sizes derived by repeatedly multiplying (or dividing) a base size by a fixed ratio. The concept was popularized for web design by **Tim Brown** (formerly of Adobe/Typekit) and **Scott Kellum**, who built the [Modular Scale](https://www.modularscale.com/) calculator. The idea draws on Robert Bringhurst's observation that "a scale is a prearranged set of harmonious proportions" (*The Elements of Typographic Style*, ch. 8).

### Named Ratios

| Name              | Ratio  | Musical/geometric origin         |
|-------------------|--------|----------------------------------|
| Minor Second      | 1.067  | 16:15 semitone                   |
| Major Second      | 1.125  | 9:8 whole tone                   |
| Minor Third       | 1.200  | 6:5                              |
| **Major Third**   | 1.250  | 5:4                              |
| **Perfect Fourth** | 1.333 | 4:3                              |
| **Augmented Fourth** | 1.414 | sqrt(2), the diagonal of a square |
| **Perfect Fifth** | 1.500  | 3:2                              |
| **Golden Ratio**  | 1.618  | (1 + sqrt(5)) / 2               |
| Octave            | 2.000  | 2:1                              |

### Deriving a Scale from a Base Size

Given a base size `B` (commonly 16px for screen) and a ratio `r`:

```
step(n) = B * r^n
```

where `n` is any integer (positive for headings, negative for small/caption text).

#### Example: Major Third (1.250) with base 16px

| Step | Calculation         | Size (px) | Typical use     |
|------|---------------------|-----------|-----------------|
| -2   | 16 / 1.25^2         | 10.24     | Fine print      |
| -1   | 16 / 1.25           | 12.80     | Caption, label   |
|  0   | 16                  | 16.00     | Body text       |
| +1   | 16 * 1.25           | 20.00     | H5 / lead text  |
| +2   | 16 * 1.25^2         | 25.00     | H4              |
| +3   | 16 * 1.25^3         | 31.25     | H3              |
| +4   | 16 * 1.25^4         | 39.06     | H2              |
| +5   | 16 * 1.25^5         | 48.83     | H1              |

#### Example: Perfect Fourth (1.333) with base 16px

| Step | Size (px) | Typical use     |
|------|-----------|-----------------|
| -2   | 9.00      | Fine print      |
| -1   | 12.00     | Caption         |
|  0   | 16.00     | Body text       |
| +1   | 21.33     | H5 / lead       |
| +2   | 28.43     | H4              |
| +3   | 37.90     | H3              |
| +4   | 50.52     | H2              |
| +5   | 67.34     | H1              |

### Choosing a Ratio

- **Tight scales** (Major Second 1.125, Minor Third 1.200): Small screens, compact UIs, mobile. Steps are subtle.
- **Medium scales** (Major Third 1.250, Perfect Fourth 1.333): Most websites, dashboards, applications. Clear hierarchy without extremes.
- **Wide scales** (Perfect Fifth 1.500, Golden Ratio 1.618): Editorial, hero-heavy layouts, large screens. Dramatic contrast between levels.

Tim Brown recommends starting with a ratio that has meaning for your design --- for example, choosing a ratio derived from two important values (a base body size and a key structural dimension). See his article [More Meaningful Typography](https://alistapart.com/article/more-meaningful-typography/) on A List Apart.

### Double-Stranded Scales

You can use **two base values** to produce a richer set of sizes. For example, base values of 16px and 20px with a Perfect Fourth ratio produce an interleaved scale with more intermediate steps, useful when a single-stranded scale feels too sparse or too tight.

### Implementation in Code

```rust
fn type_scale(base: f64, ratio: f64, step: i32) -> f64 {
    base * ratio.powi(step)
}

// Major Third scale from 16px
let sizes: Vec<f64> = (-2..=5)
    .map(|n| type_scale(16.0, 1.25, n))
    .collect();
```

---

## 2. Vertical Rhythm

Vertical rhythm is the practice of keeping all vertical spacing --- line heights, margins, padding, and element heights --- as multiples of a single base unit, producing a consistent "beat" down the page. This is the typographic equivalent of a baseline grid in print design.

### Establishing the Baseline Unit

The baseline unit is typically the **line-height of body text**. If your body text is 16px at a line-height of 1.5, the baseline unit is:

```
baseline_unit = font_size * line_height_ratio = 16 * 1.5 = 24px
```

All vertical measurements should then be multiples (or simple fractions) of 24px.

### Line-Height Guidelines

Matthew Butterick recommends line spacing of **120--145% of the point size** ([Line spacing, Practical Typography](https://practicaltypography.com/line-spacing.html)). More specifically:

| Context                          | Line-height ratio | Example (16px base) |
|----------------------------------|-------------------|----------------------|
| Dense UI / compact text          | 1.2 (120%)        | 19.2px               |
| Body text (serif)                | 1.3--1.4          | 20.8--22.4px         |
| Body text (sans-serif)           | 1.4--1.5          | 22.4--24px           |
| Large headings (28px+)           | 1.1--1.2          | tighter              |
| Display text (48px+)             | 1.0--1.1          | very tight           |

Bringhurst notes: "Sans-serif faces often need more leading than their serifed counterparts, and faces with a large x-height need more leading than those whose x-height is small" (*Elements*, 2.4.2).

### Snapping Elements to the Grid

Every element's total vertical footprint (content + padding + border + margin) should equal a whole multiple of the baseline unit:

```
total_height = content_height + padding_top + padding_bottom
             + border_top + border_bottom
             + margin_top + margin_bottom
             = N * baseline_unit   (where N is a positive integer)
```

For headings at larger sizes where the natural line-height does not align:

```
// Heading at 31.25px, line-height 1.15 = 35.9px
// Nearest multiple of 24: 48px (2 * 24)
// Add margin to compensate: 48 - 35.9 = 12.1px distributed as margin
```

### Practical Rules

1. **Set body line-height first.** This becomes your baseline unit.
2. **Margins between paragraphs** should equal one baseline unit (one blank line).
3. **Headings** should have top margin of 2x baseline and bottom margin of 1x baseline, adjusted so the total block is a grid multiple.
4. **Images and rules** should have heights (or height + margin) that are multiples of the baseline unit.
5. **When the grid breaks, rejoin it as soon as possible.** A heading that does not align perfectly should have its surrounding spacing adjusted to restore alignment for the next paragraph.

### The Relationship Between Line-Height, Margin, and Padding

- **Line-height** controls the internal leading of text lines. It is the primary vertical rhythm setter.
- **Margin** controls the space between block-level elements. Use it to separate paragraphs, headings from body text, and sections.
- **Padding** controls space between an element's content and its border/background. Used for elements with visible backgrounds or borders.
- **Key rule**: Margin and padding values should always be multiples of the baseline unit. Common values with a 24px unit: 6px (1/4), 12px (1/2), 24px (1x), 48px (2x), 72px (3x).

Sources:
- [A Guide to Vertical Rhythm (iamsteve)](https://iamsteve.me/blog/a-guide-to-vertical-rhythm)
- [8-Point Grid: Vertical Rhythm (Elliot Dahl)](https://medium.com/built-to-adapt/8-point-grid-vertical-rhythm-90d05ad95032)
- [Why Vertical Rhythm Matters (Zell Liew)](https://zellwk.com/blog/why-vertical-rhythms/)

---

## 3. Font Pairing

### Core Principles

Font pairing is governed by the tension between **contrast** (differences that create visual interest and hierarchy) and **harmony** (shared structural characteristics that make fonts feel related).

#### Bringhurst's Guidance

Bringhurst advises choosing typefaces that share an "inner structure" even when they differ in outward appearance. Fonts designed in the same historical period, or by the same designer, or for the same purpose tend to pair well. He warns against combining faces that are "almost the same" --- this creates visual confusion rather than contrast (*Elements*, 6.5).

#### Tschichold's Perspective

Jan Tschichold, in *The New Typography* (1928), advocated for a radical reduction in typeface variety: use one family (preferably sans-serif) and create hierarchy through size, weight, and spacing rather than through multiple typefaces. While his absolute position has been moderated by subsequent practice, the underlying principle --- that fewer fonts used well beats many fonts used carelessly --- remains foundational.

### Pairing Strategy: Serif + Sans-Serif + Monospace

A robust three-font system:

| Role         | Characteristics                                   | Example            |
|--------------|----------------------------------------------------|--------------------|
| **Heading**  | Sans-serif or display serif; high contrast weight  | Optima, Gill Sans  |
| **Body**     | Serif for long reading; even texture               | Palatino, Georgia  |
| **Code**     | Monospace; clear glyph distinction (0/O, 1/l/I)   | Monaco, Fira Code  |

Or invert: use a serif for headings (Didot, Playfair Display) and a sans-serif for body (system sans, Inter).

### X-Height Matching

The **x-height** is the height of the lowercase 'x' relative to the cap height. When pairing fonts:

- Fonts with similar x-height ratios appear visually compatible at the same point size.
- If x-heights differ, the font with the smaller x-height will appear smaller even at the same nominal size. Compensate by increasing its point size.
- Matching x-height is more important than matching cap height for body text, because body text is predominantly lowercase.

### Structural Properties to Compare

When evaluating potential pairs, compare:

1. **x-height ratio** (x-height / cap-height): Should be within ~5% of each other.
2. **Stroke contrast**: Both high-contrast or both low-contrast, or intentionally opposed.
3. **Axis of stress**: Vertical-stress fonts pair with other vertical-stress fonts.
4. **Letter proportions**: Condensed with condensed, wide with wide, or intentional contrast.
5. **Period / historical context**: Renaissance serif + humanist sans (both share calligraphic roots).

### Superfamilies

The safest pairing is a **superfamily** --- a typeface designed with serif, sans-serif, and monospace variants that share metrics:

- **Lucida** (Sans, Serif, Console) by Bigelow & Holmes
- **PT** (Serif, Sans, Mono) by ParaType
- **IBM Plex** (Sans, Serif, Mono) by Mike Abbink
- **Source** (Sans Pro, Serif Pro, Code Pro) by Adobe
- **Noto** (Sans, Serif, Mono) by Google

### Rules of Thumb

1. **Two fonts maximum** for most projects; three if you need monospace.
2. **Never pair two fonts from the same classification** (e.g., two geometric sans-serifs, two Didone serifs) unless they are from the same superfamily.
3. **Contrast in one dimension, harmony in others.** If fonts differ in serif/sans, they should share weight, proportion, or x-height.
4. **Test at actual sizes.** A pairing that works at 48px for headings may fail at 14px for body text.

Sources:
- [The Ultimate Guide to Font Pairing (Canva)](https://www.canva.com/learn/the-ultimate-guide-to-font-pairing/)
- [Three Secrets to Font Pairing (Adobe)](https://adobe.design/stories/leading-design/three-secrets-to-font-pairing)
- [Font Combinations Guide (Toptal)](https://www.toptal.com/designers/typography/font-combinations)

---

## 4. Hierarchy Through Typography Alone

The goal is to communicate information structure --- what is a heading, what is a label, what is body text, what is metadata --- **without relying on boxes, cards, colored backgrounds, or dividers**. This is sometimes called "naked typography" or "containerless design."

### The Six Typographic Levers

#### 4.1 Size

The most powerful differentiator. A modular scale (see Section 1) provides the numeric backbone.

- **Primary heading**: 2.5--3x body size (e.g., 40--48px if body is 16px)
- **Secondary heading**: 1.5--2x body size (e.g., 24--32px)
- **Body text**: base size (16px)
- **Supporting text / metadata**: 0.75--0.875x body (e.g., 12--14px)

#### 4.2 Weight

Font weight creates hierarchy without changing size.

| Weight value | Name        | Use                               |
|-------------|-------------|------------------------------------|
| 300         | Light       | De-emphasized, large display text  |
| 400         | Regular     | Body text                          |
| 500         | Medium      | Subtle emphasis, subheadings       |
| 600         | Semibold    | UI labels, strong subheadings      |
| 700         | Bold        | Headings, key information          |
| 800--900    | Extra/Black | Display, hero text                 |

- Use no more than **2--3 weights** from a single family.
- Heavier weights at large sizes; lighter weights risk being invisible at small sizes.

#### 4.3 Tracking (Letter-Spacing)

Tracking adjusts the uniform spacing between all characters in a word or line.

Per Butterick ([Letterspacing, Practical Typography](https://practicaltypography.com/letterspacing.html)):

- **All caps text**: add 5--12% extra letter-spacing. In CSS/code: `letter-spacing: 0.05em` to `0.12em`.
- **Small caps**: add 5--8% unless the font's small caps have built-in spacing.
- **Body text**: do not adjust. Leave at the font's default.
- **Large display text (48px+)**: optionally tighten by -1% to -2% (`letter-spacing: -0.01em` to `-0.02em`), as large text tends to appear loose.

Bringhurst's rule: "Letterspace all strings of capitals and small caps, and all long strings of digits" (*Elements*, 2.1.6). See [webtypography.net/2.1.6](http://webtypography.net/2.1.6).

#### 4.4 Case

| Case            | Effect on hierarchy          | Letter-spacing needed |
|-----------------|------------------------------|-----------------------|
| ALL CAPS        | Commands attention, labels   | +5--12%               |
| Small Caps      | Formal emphasis, acronyms    | +5--8%                |
| Title Case      | Headings                     | Default               |
| Sentence case   | Body text, natural reading   | Default               |
| lowercase       | Informal, de-emphasized      | Default               |

**All caps reduces readability** for body text because it eliminates ascender/descender variation. Use it only for short strings: labels, navigation items, category names.

#### 4.5 Opacity / Color Value

Using a single hue (e.g., black) at different opacities creates a hierarchy without introducing new colors:

| Opacity          | Hex (on white) | Use                          |
|------------------|----------------|-------------------------------|
| 100% (1.0)       | #000000        | Headings, primary content     |
| 87% (0.87)       | #212121        | Body text                     |
| 60% (0.60)       | #666666        | Secondary text, captions      |
| 38% (0.38)       | #9E9E9E        | Disabled, placeholder         |

Google's Material Design uses this pattern explicitly. On dark backgrounds, invert the pattern with white at varying opacities.

#### 4.6 Typeface Switching

Switching from serif to sans-serif (or to monospace) signals a role change:

- **Serif**: body text, long-form reading
- **Sans-serif**: headings, UI elements, labels
- **Monospace**: code, data, technical identifiers

### Combining Levers

Effective hierarchy uses **2--3 levers simultaneously**, not just one:

```
H1:     32px / Bold / Sans-serif / Sentence case / #000
H2:     24px / Semibold / Sans-serif / Sentence case / #000
H3:     20px / Medium / Sans-serif / Sentence case / #212121
Body:   16px / Regular / Serif / Sentence case / #212121
Label:  12px / Semibold / Sans-serif / ALL CAPS / #666 / +0.08em tracking
Code:   14px / Regular / Monospace / Sentence case / #212121
```

Sources:
- [Typographic Hierarchies (Smashing Magazine)](https://www.smashingmagazine.com/2022/10/typographic-hierarchies/)
- [How to Structure an Effective Typographic Hierarchy (Toptal)](https://www.toptal.com/designers/typography/typographic-hierarchy)
- [Typography in Design Systems (Nathan Curtis / EightShapes)](https://medium.com/eightshapes-llc/typography-in-design-systems-6ed771432f1e)

---

## 5. Measure (Line Length)

### The Fundamental Rule

Bringhurst states: "Anything from 45 to 75 characters is widely regarded as a satisfactory length of line for a single-column page set in a serifed text face in a text size. The 66-character line (counting both letters and spaces) is widely regarded as ideal. For multiple columns, a better average is 40 to 50 characters" (*Elements of Typographic Style*, 2.1.2).

Butterick concurs, recommending **45--90 characters** per line including spaces ([Summary of Key Rules, Practical Typography](https://practicaltypography.com/summary-of-key-rules.html)). His wider range accommodates sans-serif faces and screen reading.

### Specific Values

| Context                  | Characters/line | Notes                           |
|--------------------------|----------------|---------------------------------|
| Single-column body text  | 45--75         | 66 ideal (Bringhurst)          |
| Multi-column layout      | 40--50         | Narrower to aid column scanning |
| Wide screen / sans-serif | 45--90         | Butterick's extended range      |
| Code / monospace         | 80--120        | Convention from terminal widths  |

### Calculating Measure from Font Size

A rough approximation: for most proportional text faces at a given `font-size`, the average character width is approximately **0.5em** (varies by typeface). Therefore:

```
measure = target_characters * average_char_width
        = 66 * 0.5em
        = 33em
```

Common `max-width` values for text containers:

| Target chars | max-width (em) | max-width (px at 16px) |
|-------------|----------------|------------------------|
| 45          | ~22em          | ~352px                 |
| 66          | ~33em          | ~528px                 |
| 75          | ~37em          | ~592px                 |
| 90          | ~45em          | ~720px                 |

These are approximations. Always test with your specific typeface by counting actual characters.

### Relationship to Line-Height

Bringhurst and Lupton both note the coupling between line length and line-height:

- **Longer lines need more line-height** to help the eye track back to the start of the next line.
- **Shorter lines tolerate tighter line-height** because the return sweep is short.

| Measure          | Recommended line-height |
|------------------|------------------------|
| Short (40--50ch) | 1.2--1.35              |
| Medium (50--70ch)| 1.35--1.5              |
| Long (70--90ch)  | 1.5--1.6               |

### Why This Matters

When lines are too long (100+ characters), readers lose their place when returning to the left margin. When lines are too short (below 40 characters), the eye is forced into constant line-breaks, reading becomes choppy, and excessive hyphenation may be required.

Sources:
- [The Elements of Typographic Style (Bringhurst)](https://en.wikipedia.org/wiki/The_Elements_of_Typographic_Style)
- [Summary of Key Rules (Butterick)](https://practicaltypography.com/summary-of-key-rules.html)
- [Line Length & Measure (Type & Music)](http://typeandmusic.com/line-lengths-measures/)

---

## 6. Microtypography

Microtypography concerns the fine-grained adjustments that separate professional typography from default rendering. These details are "subliminal refinements towards typographical perfection" (Han The Thanh, pdfTeX documentation).

### 6.1 Kerning

**Kerning** adjusts the space between specific pairs of characters to achieve optically even spacing. Common problematic pairs: AV, AW, AY, AT, LT, LV, LY, To, Tr, Ta, Yo, Wa, We.

- Most professional fonts include kerning tables (OpenType `kern` feature).
- In CSS: `font-kerning: normal` (enabled by default in most browsers).
- In code rendering: enable the `kern` OpenType feature via font shaping (HarfBuzz, CoreText).
- Butterick: "Kerning should always be turned on" ([Kerning, Practical Typography](https://practicaltypography.com/kerning.html)).

### 6.2 Ligatures

**Ligatures** replace problematic character combinations with single glyphs designed to resolve spacing or collision issues.

| Type              | Examples      | OpenType feature | Usage                          |
|-------------------|---------------|------------------|--------------------------------|
| Standard          | fi, fl, ff, ffi, ffl | `liga`     | Always enable for body text    |
| Discretionary     | ct, st, Th    | `dlig`           | Use selectively; editorial     |
| Historical        | long s combos | `hlig`           | Rarely used outside facsimile  |

- In CSS: `font-variant-ligatures: common-ligatures` (default in modern browsers).
- **Do not use ligatures in monospace text** --- they undermine the fixed-width grid.
- **Do not use ligatures across morpheme boundaries** in some contexts (e.g., "shelflife" should not ligate the f-l if it's "shelf-life").

### 6.3 Hanging Punctuation

Punctuation marks at the edges of text blocks should **hang into the margin** so that the text edge appears optically straight. Characters affected: quotation marks (" " ' '), periods, commas, hyphens, dashes, parentheses, brackets.

- In CSS: `hanging-punctuation: first last` (Safari only as of 2025; other browsers require JavaScript workarounds).
- In print/PDF: pdfTeX `\microtypesetup{protrusion=true}`, or InDesign's Optical Margin Alignment.
- Bringhurst: "Hang the punctuation in the margin to maintain the integrity of the text block" (*Elements*, 2.1).

For non-Safari web implementations, a common workaround is to apply a negative `text-indent` to paragraphs that begin with a quotation mark.

### 6.4 Optical Margin Alignment

Beyond hanging punctuation, certain letterforms that are visually narrower (A, V, W, Y, T, round letters like O, C) can be slightly protruded into the margin for a cleaner visual edge. This is called **protrusion** or **margin kerning**.

Typical protrusion values:

| Character type    | Protrusion amount |
|-------------------|-------------------|
| Period, comma     | 100% of width     |
| Hyphen, en-dash   | 70--80%           |
| Quotation marks   | 70--100%          |
| Round letters (O) | 5--15%            |
| Pointed (A, V, W) | 5--15%            |
| T, L              | 5--10%            |

### 6.5 Figure Styles

Professional typefaces offer multiple numeral designs via OpenType features:

| Style                      | Feature | Characteristics                    | Use case                         |
|----------------------------|---------|------------------------------------|----------------------------------|
| **Oldstyle / text figures**| `onum`  | Varying heights, some descend      | Body text (blend with lowercase) |
| **Lining / titling figures**| `lnum` | Uniform cap height                 | Tables, all-caps text, headings  |
| **Proportional**           | `pnum`  | Variable width per digit           | Running text                     |
| **Tabular**                | `tnum`  | Fixed width per digit              | Tables, columns, aligned numbers |

**Combination rules:**

- Body text: **oldstyle proportional** (`onum` + `pnum`) --- digits harmonize with lowercase letters.
- Headings / all-caps: **lining proportional** (`lnum` + `pnum`) --- digits match cap height.
- Tables / data: **lining tabular** (`lnum` + `tnum`) --- digits align vertically in columns.
- Small caps context: **oldstyle** figures, which Bringhurst says "harmonize with small caps" (*Elements*, 3.2.1).

In CSS:
```css
/* Oldstyle proportional for body */
body { font-variant-numeric: oldstyle-nums proportional-nums; }

/* Lining tabular for tables */
table { font-variant-numeric: lining-nums tabular-nums; }
```

### 6.6 Small Caps

True **small caps** are not simply scaled-down capitals --- they are separately designed glyphs with stroke weights matched to the lowercase. Faked small caps (CSS `font-variant: small-caps` without an actual small caps font) produce thin, anemic letterforms.

Rules:
- **Always letterspace small caps** by 5--8% (Bringhurst, Butterick).
- Use small caps for: abbreviations (NASA, FBI), acronyms in body text, AM/PM, era designations (AD, BC).
- In CSS: `font-variant-caps: all-small-caps` or `font-feature-settings: "smcp"`.
- Test that the font has true small caps glyphs (`smcp` feature) before enabling.

Sources:
- [Microtypography (Wikipedia)](https://en.wikipedia.org/wiki/Microtypography)
- [Micro-Typography: Spacing and Kerning (Smashing Magazine)](https://www.smashingmagazine.com/2020/05/micro-typography-space-kern-punctuation-marks-symbols/)
- [Hanging Punctuation (CSS-Tricks)](https://css-tricks.com/almanac/properties/h/hanging-punctuation/)
- [Alternate Figures (Butterick)](https://practicaltypography.com/alternate-figures.html)
- [Optical Margin Alignment (Wikipedia)](https://en.wikipedia.org/wiki/Optical_margin_alignment)

---

## Quick Reference: Key Numbers for Implementation

### Font Sizes (16px base, Major Third 1.250)

```
fine-print:  10.24px   (step -2)
caption:     12.80px   (step -1)
body:        16.00px   (step  0)
h5/lead:     20.00px   (step +1)
h4:          25.00px   (step +2)
h3:          31.25px   (step +3)
h2:          39.06px   (step +4)
h1:          48.83px   (step +5)
display:     61.04px   (step +6)
```

### Line-Height

```
body text:       1.4--1.5    (serif: 1.3--1.4, sans: 1.4--1.5)
headings:        1.1--1.25
display (48px+): 1.0--1.1
captions:        1.3--1.4
```

### Baseline Unit

```
With 16px body at 1.5 line-height: baseline = 24px
All spacing in multiples: 6, 12, 24, 48, 72, 96
```

### Letter-Spacing

```
body text:       0 (default)
all caps:        +0.05em to +0.12em
small caps:      +0.05em to +0.08em
large display:   -0.01em to -0.02em
```

### Line Length (Measure)

```
ideal:           66 characters (~33em)
acceptable:      45--75 characters (~22--37em)
multi-column:    40--50 characters (~20--25em)
```

### Butterick's Core Numbers

```
print body size:    10--12pt
screen body size:   15--25px
line spacing:       120--145% of font size
line length:        45--90 characters
```

---

## Primary Sources and References

### Books
- Bringhurst, Robert. *The Elements of Typographic Style*, 4th ed. Hartley & Marks, 2012.
- Tschichold, Jan. *The New Typography* (Die Neue Typographie), 1928. Trans. Ruari McLean, UC Press, 2006.
- Lupton, Ellen. *Thinking with Type*, 2nd ed. Princeton Architectural Press, 2010.

### Online
- Butterick, Matthew. *Practical Typography*. [https://practicaltypography.com/](https://practicaltypography.com/)
  - [Summary of Key Rules](https://practicaltypography.com/summary-of-key-rules.html)
  - [Line Spacing](https://practicaltypography.com/line-spacing.html)
  - [Letterspacing](https://practicaltypography.com/letterspacing.html)
  - [Kerning](https://practicaltypography.com/kerning.html)
  - [Alternate Figures](https://practicaltypography.com/alternate-figures.html)
- Brown, Tim. "More Meaningful Typography." *A List Apart*. [https://alistapart.com/article/more-meaningful-typography/](https://alistapart.com/article/more-meaningful-typography/)
- Brown, Tim and Kellum, Scott. *Modular Scale*. [https://www.modularscale.com/](https://www.modularscale.com/)
- *The Elements of Typographic Style Applied to the Web*. [http://webtypography.net/](http://webtypography.net/)
- Smashing Magazine. "Typographic Hierarchies." [https://www.smashingmagazine.com/2022/10/typographic-hierarchies/](https://www.smashingmagazine.com/2022/10/typographic-hierarchies/)
- Smashing Magazine. "Micro-Typography." [https://www.smashingmagazine.com/2020/05/micro-typography-space-kern-punctuation-marks-symbols/](https://www.smashingmagazine.com/2020/05/micro-typography-space-kern-punctuation-marks-symbols/)
