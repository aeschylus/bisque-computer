# Print Layout Design Principles and Grid Systems

Research on classical page proportions, grid systems, whitespace, and print-to-screen
translation, with concrete numeric values for implementation.

---

## 1. Classical Page Proportions

### 1.1 Van de Graaf Canon

The Van de Graaf canon is a geometric method for positioning the text area on a page
that works for **any page ratio**. It was described by J.A. van de Graaf and later
popularized by Jan Tschichold.

**The Construction (step by step):**

1. Draw the two-page spread (verso and recto).
2. Draw diagonals from the bottom-left to top-right of each page.
3. Draw a diagonal from the bottom-left corner of the left page to the top-right corner
   of the right page (the "double-page diagonal").
4. Where the double-page diagonal intersects the single-page diagonal on each page, draw
   a vertical line upward to the top edge.
5. From where that vertical meets the top edge, draw a line to the bottom-left corner of
   the same page.
6. The intersections of these lines define the corners of the text block.

**Resulting proportions:**

The construction always produces a 9x9 grid, regardless of page dimensions:

- **Inner (gutter) margin:** 1/9 of page width
- **Top margin:** 1/9 of page height
- **Outer margin:** 2/9 of page width
- **Bottom margin:** 2/9 of page height

For a 2:3 page ratio, this yields the margin ratio **2:3:4:6** (inner:top:outer:bottom).

More generally, for a page ratio of 1:R, the margin ratio is **1:R:2:2R**.

**Key property:** The text block has the same proportions as the page itself. The height
of the text block equals the width of the page.

**Why it works aesthetically:** The text block occupies exactly the same proportional
position on every page. The asymmetric margins create visual tension that draws the eye
inward. The larger outer and bottom margins provide "weight" at the base and a natural
resting place for the thumb when holding a book.

**Concrete numbers for a 6x9 inch page (common book format):**

| Element | Calculation | Value |
|---------|-------------|-------|
| Inner margin | 6/9 | 0.667 in |
| Top margin | 9/9 | 1.000 in |
| Outer margin | 12/9 | 1.333 in |
| Bottom margin | 18/9 | 2.000 in |
| Text width | 6 - 0.667 - 1.333 | 4.000 in |
| Text height | 9 - 1.000 - 2.000 | 6.000 in |

Note: text height (6.000) = page width (6.000). Text block ratio = 4:6 = 2:3 = page ratio.

Sources:
- [Canons of page construction - Wikipedia](https://en.wikipedia.org/wiki/Canons_of_page_construction)
- [The Secret Law of Page Harmony - Retinart](https://retinart.net/graphic-design/secret-law-of-page-harmony/)
- [Van de Graaf Canon adaptation - Medium](https://medium.com/design-bootcamp/grid-system-van-de-graaf-canon-adaptation-21a710181336)

---

### 1.2 Tschichold's Golden Canon

Jan Tschichold coined "the Golden Canon of book page construction" in 1953, describing
the proportional system used in the finest late-Gothic manuscripts. This is **not** related
to the golden ratio (phi = 1.618); the name refers to the "golden age" of manuscript
production.

**Core principles:**

1. The page proportion is **2:3** (the same ratio as medieval codices).
2. The text block has the **same proportion as the page** (2:3).
3. The **height of the text block equals the width of the page**.
4. Margin proportions are **1:1:2:3** (inner:top:outer:bottom) in Tschichold's original
   description, or equivalently **2:3:4:6** when the Van de Graaf construction is applied
   to a 2:3 page.

**The relationship between page ratio and text block ratio:**

Tschichold proved that in the ideal case, the text area must have the same proportions as
the page. If the page is W x H, then the text area is also W' x H' where W'/H' = W/H.
The crowning insight: the text block height H' = W (the page width).

**Medieval proportions Tschichold documented:**

| Property | Value |
|----------|-------|
| Page ratio | 2:3 |
| Text block ratio | 2:3 |
| Text block area | 44.4% of page area |
| Inner margin | 1/9 page width |
| Top margin | 1/9 page height |
| Outer margin | 2/9 page width |
| Bottom margin | 2/9 page height |

Sources:
- [Canons of page construction - Wikipedia](https://en.wikipedia.org/wiki/Canons_of_page_construction)
- [The "Golden Canon" of Book-Page Design (Bridges 2012 paper)](https://archive.bridgesmathart.org/2012/bridges2012-417.pdf)
- [ResearchGate: The 'Golden Canon' of book-page construction](https://www.researchgate.net/publication/254253079_The_'Golden_Canon'_of_book-page_construction_Proving_the_proportions_geometrically)

---

### 1.3 Villard de Honnecourt's Diagram

Villard de Honnecourt was a 13th-century French architect whose sketchbook (c. 1235)
contained a geometric system for dividing any rectangle into harmonious parts. Tschichold
brought this system to typographic attention in 1955.

**The 9-part division:**

Villard's method divides each edge of the page into **9 equal segments**, creating
an 81-cell grid (9 x 9). The text block is placed so that:

- It begins at 1/9 from the top and 1/9 from the inner edge
- It ends at 7/9 from the top (leaving 2/9 at the bottom) and 7/9 from the inner edge
  (leaving 2/9 at the outer edge)

The text block therefore occupies **2/3 of the width and 2/3 of the height**, which is
(2/3)^2 = **4/9 = 44.4% of the total page area**.

**Villard's Figure (line division):**

Beyond the 9-part grid, Villard developed a general system for dividing a line segment
into any number of equal parts (thirds, fourths, fifths, etc.) using only a straightedge
and compass. This recursive division creates **nested harmonious proportions**.

**Margin proportions produced:**

| Direction | Inner/Top | Outer/Bottom | Ratio |
|-----------|-----------|--------------|-------|
| Horizontal | 1/9 page width | 2/9 page width | 1:2 |
| Vertical | 1/9 page height | 2/9 page height | 1:2 |

Sources:
- [Canons of page construction - Wikipedia](https://en.wikipedia.org/wiki/Canons_of_page_construction)
- [The Secret Law of Page Harmony - Retinart](https://retinart.net/graphic-design/secret-law-of-page-harmony/)
- [History of the design grid - 99designs](https://99designs.com/blog/tips/history-of-the-grid-part-1/)

---

### 1.4 The Renard Series

The Renard series was proposed by French army engineer Colonel Charles Renard around 1877
and standardized as ISO 3. It provides **preferred number sequences** that divide the
interval 1-10 into geometrically spaced steps.

**The series:**

| Series | Steps per decade | Step factor | Approx. ratio between steps |
|--------|-----------------|-------------|---------------------------|
| R5 | 5 | 10^(1/5) = 1.585 | ~58% increase |
| R10 | 10 | 10^(1/10) = 1.259 | ~26% increase |
| R20 | 20 | 10^(1/20) = 1.122 | ~12% increase |
| R40 | 40 | 10^(1/40) = 1.059 | ~6% increase |
| R80 | 80 | 10^(1/80) = 1.029 | ~3% increase |

**Concrete values:**

```
R5:  1.00, 1.60, 2.50, 4.00, 6.30, 10.00
R10: 1.00, 1.25, 1.60, 2.00, 2.50, 3.15, 4.00, 5.00, 6.30, 8.00, 10.00
R20: 1.00, 1.12, 1.25, 1.40, 1.60, 1.80, 2.00, 2.24, 2.50, 2.80,
     3.15, 3.55, 4.00, 4.50, 5.00, 5.60, 6.30, 7.10, 8.00, 9.00, 10.00
```

**Application to design spacing:**

The Renard series can generate a harmonious spacing scale. If your base unit is 8px:

```
R5-based scale:  8, 13, 20, 32, 50, 80
R10-based scale: 8, 10, 13, 16, 20, 25, 32, 40, 50, 64, 80
```

The key advantage over linear scales (8, 16, 24, 32...) is that Renard values maintain
**constant perceived proportional differences** between steps, because human perception
of size is logarithmic, not linear.

Sources:
- [Renard series - Wikipedia](https://en.wikipedia.org/wiki/Renard_series)
- [Preferred number - Wikipedia](https://en.wikipedia.org/wiki/Preferred_number)
- [Renard's Preferred Numbers - Oughtred Society Journal](https://osgalleries.org/journal/pdf_files/13.1/V13.1P44.pdf)

---

## 2. Grid Systems

### 2.1 Josef Muller-Brockmann and the Swiss/International Style

Josef Muller-Brockmann (1914-1996) formalized the grid as the primary structural tool of
graphic design in his 1981 book *Grid Systems in Graphic Design*.

**Core principles of the Swiss grid:**

1. **The grid is invisible.** It creates order without being seen. Content aligns to it;
   the grid itself is never rendered.
2. **Mathematical precision.** All elements (text, images, whitespace) align to the grid.
   Typography sits on a baseline grid; images snap to column edges.
3. **Reduction to essentials.** Sans-serif type (Helvetica, Akzidenz-Grotesk), photography
   over illustration, asymmetric layouts.
4. **Objective communication.** The grid removes subjective design decisions and replaces
   them with systematic structure.

**Muller-Brockmann's grid construction method:**

1. Define the page/format dimensions.
2. Define margins (using proportional systems like the canons above).
3. Divide the remaining space into columns with gutters.
4. Establish a baseline grid from the body text leading.
5. All elements align to column edges horizontally and baseline grid vertically.

**His recommended approach to margins:** Margins should be "visually interesting and
functional" -- he advocated for margins that are proportionally related to the column
width and gutter, not arbitrary.

Sources:
- [Josef Muller-Brockmann: Pioneer of Swiss Graphic Design - designyourway.net](https://www.designyourway.net/blog/josef-muller-brockmann/)
- [Josef Muller-Brockmann: Principal of the Swiss School - Jotform](https://www.jotform.com/blog/josef-muller-brockmann-principal-of-the-swiss-school/)
- [Grid Systems in Graphic Design (Archive.org PDF)](https://ia802309.us.archive.org/4/items/GridSystemsInGraphicDesignJosefMullerBrockmann/Grid%20systems%20in%20graphic%20design%20-%20Josef%20Muller-Brockmann.pdf)

---

### 2.2 Multi-Column Grids

Multi-column grids divide the content area into vertical columns separated by gutters.

**Common column counts and their divisibility:**

| Columns | Divisible by | Possible sub-layouts |
|---------|-------------|---------------------|
| 3 | 1, 3 | 1-col, 3-col |
| 4 | 1, 2, 4 | 1-col, 2-col, 4-col |
| 6 | 1, 2, 3, 6 | 1, 2, 3, 6-col |
| 12 | 1, 2, 3, 4, 6, 12 | 1, 2, 3, 4, 6, 12-col |

**12 columns** is the most versatile: it supports the widest range of subdivisions.

**Column width formula:**

```
column_width = (content_width - (num_columns - 1) * gutter_width) / num_columns
```

Or equivalently:

```
content_width = (num_columns * column_width) + ((num_columns - 1) * gutter_width)
```

**Common gutter widths:**

| System | Gutter | Base unit |
|--------|--------|-----------|
| Bootstrap 4 | 30px (15px each side) | -- |
| Bootstrap 5 | 24px (1.5rem) | 16px |
| Material Design | 16px or 24px | 8px |
| CMS Design System | 16px | 8px |

**Responsive column counts:**

| Viewport | Columns | Typical margins |
|----------|---------|-----------------|
| Mobile (<600px) | 4 | 16-20px |
| Tablet (600-1024px) | 8 | 24-32px |
| Desktop (>1024px) | 12 | 24-64px |

**Concrete example: 12-column grid at 1200px content width, 24px gutters:**

```
content_width = 1200px
gutters = 11 * 24px = 264px
total_column_width = 1200 - 264 = 936px
single_column = 936 / 12 = 78px

So: 12 columns of 78px with 11 gutters of 24px = 1200px
```

Sources:
- [Bootstrap Grid System](https://getbootstrap.com/docs/4.0/layout/grid/)
- [Layout Grid - CMS Design System](https://design.cms.gov/foundation/layout-grid/layout-grid/)
- [Spacing, grids, and layouts - designsystems.com](https://www.designsystems.com/space-grids-and-layouts/)
- [Grid Layouts in Web Design - Elementor](https://elementor.com/blog/grid-design/)

---

### 2.3 Modular Grids

A modular grid adds **horizontal flowlines** to a column grid, creating a matrix of
rectangular **modules** (cells).

**Components:**

- **Columns:** Vertical divisions (as in column grids)
- **Rows:** Horizontal divisions defined by flowlines
- **Modules:** The individual cells formed by column/row intersections
- **Spatial zones:** Groups of adjacent modules that can be combined for larger elements
- **Flowlines:** Horizontal lines that create rows; guide the eye across the page
- **Gutters:** Spacing between both columns and rows

**How to derive row height from baseline grid:**

If body text is 16px with 24px line-height (1.5x), the baseline grid increment is 24px.
Rows should span a whole number of baseline increments:

```
row_height = N * baseline_increment

Example: 10 baselines per row
row_height = 10 * 24px = 240px
```

**Typical modular grid for a dashboard layout (1440px wide):**

```
Margins:       64px each side
Content width: 1312px
Columns:       12
Column width:  78px
Column gutter: 24px
Row height:    120px (5 baselines at 24px)
Row gutter:    24px
```

**Use cases:** Dashboards, image galleries, card layouts, data tables, calendars,
e-commerce product grids.

Sources:
- [Layout Design: Types of Grids - Visme](https://visme.co/blog/layout-design/)
- [4 Types of Grids and When Each Works Best - Vanseo Design](https://vanseodesign.com/web-design/grid-types/)
- [Anatomy of a Modular Typographic Grid - Vanseo Design](https://vanseodesign.com/web-design/grid-anatomy/)
- [Modular grids for visually appealing layouts - Stuff & Nonsense](https://stuffandnonsense.co.uk/blog/modular-grids-for-visually-appealing-layouts)

---

### 2.4 Compound Grids

A compound grid overlays two or more grids on the same page to create more complex and
dynamic layouts.

**How compound grids work:**

Take two column grids with **different column counts** and overlay them. The result has
more alignment points than either grid alone, creating a richer rhythmic structure.

**Example: 4+5 compound grid**

Overlaying a 4-column grid and a 5-column grid on the same content area produces a
compound grid with **8 columns of 4 different widths**, in this rhythmic pattern:

```
Column widths (in proportional units): 6 | 1 | 4 | 3 | 3 | 4 | 1 | 6
```

This is far more interesting than a uniform 12-column grid because it creates:
- Narrow columns for captions or annotations
- Wide columns for primary content
- Asymmetric pairings that generate visual interest

**When to use compound grids:**

- Magazine layouts with varied content types
- Editorial websites mixing text, images, and pull quotes
- Layouts that need to feel dynamic without losing structure
- When a uniform grid feels too rigid or monotonous

**Implementation approach:**

1. Define two simple grids independently.
2. Mark all column edges from both grids on the same axis.
3. The union of all edges defines the compound grid's columns.
4. Use CSS Grid with explicit track definitions matching the compound pattern.

Sources:
- [Using a 4+5 compound grid - Stuff & Nonsense](https://stuffandnonsense.co.uk/blog/using-a-4-5-compound-grid)
- [Get started quickly with a 4+5 compound grid - Stuff & Nonsense](https://stuffandnonsense.co.uk/blog/get-started-quickly-with-a-4-5-compound-grid)
- [Inspired Design Decisions: Pressing Matters - Smashing Magazine](https://www.smashingmagazine.com/2019/07/inspired-design-decisions-pressing-matters/)

---

## 3. Whitespace as Structure

### 3.1 Active vs. Passive Whitespace

**Active whitespace** is intentionally added to create visual structure:
- Separates groups of related content (Gestalt law of proximity)
- Directs the reader's eye through a visual hierarchy
- Creates emphasis by isolating elements
- Examples: generous margins around a heading, space between navigation and content,
  padding inside a card component

**Passive whitespace** is the incidental byproduct of layout decisions:
- The natural space between words and letters (tracking, kerning)
- Line-height within a text block
- Space at the end of a short line in a paragraph
- The gap left by a column that doesn't fill its height

**Design principle:** Passive whitespace should be **consistent and predictable** (via
baseline grids and spacing tokens). Active whitespace should be **deliberate and
proportional** -- scaled to the importance of the separation it creates.

**Hierarchy of active whitespace (typical ratios):**

| Level | Purpose | Multiplier |
|-------|---------|-----------|
| Micro | Between related elements within a group | 1x base |
| Small | Between items in a list or form | 2x base |
| Medium | Between content sections | 3-4x base |
| Large | Between major page regions | 6-8x base |
| Macro | Page margins, hero spacing | 8-16x base |

Sources:
- [The Power of White Space in Design - IxDF](https://www.interaction-design.org/literature/article/the-power-of-white-space)
- [White Space in Graphic Design - Zeka Design](https://www.zekagraphic.com/white-space-in-graphic-design/)

---

### 3.2 Margins as Frames

Generous margins function like a picture frame: they **focus attention inward** toward
the content.

**Principles:**

1. **Quality signal.** Wide margins historically indicated that the publisher could afford
   to "waste" paper. In digital design, generous margins signal quality and intentionality.
2. **Cognitive rest.** The eye needs a buffer zone at the edges. Without it, content feels
   cramped and anxious.
3. **Directional focus.** Asymmetric margins (larger at bottom and outside) create a
   natural focal point slightly above center, which aligns with the optical center of a
   rectangle (about 3/8 from the top, not 1/2).
4. **Proportional scaling.** As format size increases, margins should increase
   proportionally -- or even slightly more than proportionally -- to maintain the framing
   effect.

**Recommended margin sizes for screen:**

| Context | Margin as % of viewport width |
|---------|-------------------------------|
| Mobile (reading) | 5-8% |
| Tablet (reading) | 10-15% |
| Desktop (reading) | 15-25% |
| Desktop (dashboard) | 3-5% |
| Luxury/editorial | 20-30% |

Sources:
- [What is Margin in Design: The Power of White Space - designyourway.net](https://www.designyourway.net/blog/what-is-margin-in-design/)
- [Book Printing: How the Margins of a Book Enhance Readability - Color Vision Printing](https://www.colorvisionprinting.com/blog/book-printing-how-the-margins-of-a-book-enhance-readability)

---

### 3.3 The Role of the Gutter

Gutters separate columns **while maintaining visual unity** across them.

**Functions:**

1. **Prevent text collision.** Without gutters, adjacent columns of text would run
   together and become unreadable.
2. **Guide the eye downward.** A well-sized gutter encourages the reader to scan down a
   column rather than jumping across to the next one.
3. **Create rhythm.** Consistent gutter width across all columns establishes a spatial
   rhythm that the eye learns to expect.

**Sizing guidelines:**

| Body text size | Recommended gutter | Ratio to line-height |
|----------------|-------------------|---------------------|
| 14-16px | 16-20px | ~0.8-1.0x |
| 18-20px | 20-28px | ~0.8-1.0x |
| 24px+ | 24-36px | ~0.75-1.0x |

**Rule of thumb:** Gutter width should be approximately equal to the line-height of the
body text, or slightly less. It should be **wider than the word spacing** within a line
(so columns read as separate) but **narrower than the margins** (so the content area
reads as unified).

Sources:
- [Gutter - PrintWiki](https://printwiki.org/Gutter)
- [Spacing, grids, and layouts - designsystems.com](https://www.designsystems.com/space-grids-and-layouts/)
- [Gutters in Experimental Typography - numberanalytics.com](https://www.numberanalytics.com/blog/ultimate-guide-gutters-experimental-typography)

---

### 3.4 Whitespace Ratios

Typographic spacing follows a **hierarchy of distances** that reinforces the content
hierarchy.

**Recommended spacing ratios (relative to base unit):**

```
Letter spacing (tracking):   0.02-0.05em (passive)
Word spacing:                0.25em (default, passive)
Line-height:                 1.4-1.6x font size
Paragraph spacing:           0.5-1.5x line-height
Heading margin-bottom:       0.5-1.0x line-height
Heading margin-top:          1.5-2.5x line-height (more above than below)
Section spacing:             2-4x paragraph spacing
Page-level spacing:          4-8x paragraph spacing
```

**The 1.5x rule for heading proximity:**

Headings should have at least **1.5x more space above** than below, so they visually
"belong to" the content they introduce rather than the content above.

**An 8px-based spacing scale:**

```
Token     Value     Use case
--------------------------------------------------
space-1   4px       Icon-to-label gap, tight insets
space-2   8px       Inline element spacing, small padding
space-3   12px      Form field gap (4px half-step)
space-4   16px      Standard padding, gutter (small)
space-5   24px      Standard gutter, section padding
space-6   32px      Card padding, medium section gap
space-7   48px      Large section gap
space-8   64px      Page margin, major region gap
space-9   96px      Hero section padding
space-10  128px     Maximum page margin
```

This uses a **non-linear scale** (roughly following 4, 8, 12, 16, 24, 32, 48, 64, 96,
128) that provides fine control at small sizes and coarser jumps at large sizes. The
progression loosely follows: x1, x2, x3, x4, x6, x8, x12, x16, x24, x32 of a 4px
base.

Sources:
- [Line spacing - Butterick's Practical Typography](https://practicaltypography.com/line-spacing.html)
- [Typography spacing - BuninUX](https://buninux.com/learn/typography-spacing)
- [Spacing, grids, and layouts - designsystems.com](https://www.designsystems.com/space-grids-and-layouts/)
- [Harmonious spacing system - Marvel Blog](https://marvelapp.com/blog/harmonious-spacing-system-faster-design-dev-handoff/)

---

## 4. Print-to-Screen Translation

### 4.1 Points to Pixels

**The conversion:**

```
1 point (pt) = 1/72 inch
1 pixel (px) at 96 DPI = 1/96 inch

Therefore: 1pt = 96/72 px = 1.333px
Or:        1px = 72/96 pt = 0.75pt
```

**Why screen type needs to be larger than print:**

- Print body text: typically 10-12pt (effective ~13-16px equivalent)
- Screen body text: typically 16-20px (12-15pt equivalent)
- The browser default of 16px was chosen for readability at typical desktop viewing
  distance (~24 inches vs ~14 inches for print)

**Conversion table:**

| Print (pt) | Screen (px) | Typical use |
|-----------|-------------|-------------|
| 8pt | 11px | Fine print (avoid on screen) |
| 9pt | 12px | Captions, footnotes |
| 10pt | 13px | Minimum readable body (screen) |
| 11pt | 15px | Small body text |
| 12pt | 16px | Standard body text (browser default) |
| 14pt | 19px | Large body / small heading |
| 18pt | 24px | H3-level heading |
| 24pt | 32px | H2-level heading |
| 36pt | 48px | H1-level heading |
| 48pt | 64px | Display heading |
| 72pt | 96px | Hero text |

**DPI considerations:**

| Context | DPI | 1pt = |
|---------|-----|-------|
| macOS standard | 72 | 1px |
| Windows standard | 96 | 1.333px |
| macOS Retina | 144 | 2px (rendered as 1 "point") |
| Print (laser) | 300 | 4.167 dots |
| Print (offset) | 600+ | 8.333+ dots |

Sources:
- [Point (typography) - Wikipedia](https://en.wikipedia.org/wiki/Point_(typography))
- [Point size - Butterick's Practical Typography](https://practicaltypography.com/point-size.html)
- [Font size conversion - websemantics.uk](https://websemantics.uk/tools/font-size-conversion-pixel-point-em-rem-percent/)

---

### 4.2 Reading Distance

Different media are consumed at different distances, which fundamentally affects how large
type and spacing need to be.

**Typical reading distances:**

| Medium | Distance | Angular size needed |
|--------|----------|-------------------|
| Print (book/magazine) | 14-16 in (35-40 cm) | 10-12pt body |
| Desktop monitor | 20-28 in (50-70 cm) | 16-20px body |
| Laptop | 18-24 in (45-60 cm) | 16-18px body |
| Tablet (held) | 12-16 in (30-40 cm) | 16-18px body |
| Mobile phone | 10-14 in (25-35 cm) | 16px body (minimum) |

**The scaling principle:**

To maintain the same apparent (angular) size of text at different distances:

```
screen_size = print_size * (screen_distance / print_distance)

Example:
  12pt print text at 14 inches
  Viewed on screen at 24 inches
  Equivalent screen size = 12pt * (24/14) = 20.6pt ~ 27px
```

This is why 16px (12pt) on screen feels comparable to 10pt in print -- the greater
viewing distance compensates.

**Line length also scales with distance:**

Optimal reading line length is 45-75 characters (roughly 34em / 544px at 16px). This
holds across media because comfortable eye movement angles are consistent.

Sources:
- [Does Print Size Matter for Reading? - PMC/NIH](https://pmc.ncbi.nlm.nih.gov/articles/PMC3428264/)
- [Point size - Butterick's Practical Typography](https://practicaltypography.com/point-size.html)

---

### 4.3 Resolution Independence

Designing in relative units ensures layouts scale correctly across different screen
densities and user preferences.

**Unit reference:**

| Unit | Relative to | Use case |
|------|-------------|----------|
| px | Device pixel (at 1x) | Borders, shadows, fine detail |
| em | Parent element font size | Component-scoped spacing |
| rem | Root element font size | Global typography, layout spacing |
| % | Parent element dimension | Fluid widths |
| vw/vh | Viewport width/height | Full-screen layouts |
| ch | Width of "0" character | Setting line length |

**Best practices:**

1. **Set root font size in px** (or leave browser default of 16px). All other sizes
   derive from this.
2. **Use rem for font sizes and vertical spacing.** This respects user browser settings
   for accessibility.
3. **Use em for component-internal spacing** (padding, margins within a component that
   should scale with the component's own font size).
4. **Use ch for line length:** `max-width: 65ch` constrains a text block to ~65 characters
   wide regardless of font size.
5. **Use vw/vh sparingly** -- for hero sections and full-bleed layouts, not for text
   sizing (which should respond to user preferences, not viewport size alone).

**A rem-based type scale (using a 1.25 / Major Third ratio):**

```
--text-xs:    0.64rem   (10.24px)
--text-sm:    0.80rem   (12.80px)
--text-base:  1.00rem   (16.00px)
--text-lg:    1.25rem   (20.00px)
--text-xl:    1.563rem  (25.00px)
--text-2xl:   1.953rem  (31.25px)
--text-3xl:   2.441rem  (39.06px)
--text-4xl:   3.052rem  (48.83px)
--text-5xl:   3.815rem  (61.04px)
```

Sources:
- [Responsive typography with REMs - Bugsnag](https://www.bugsnag.com/blog/responsive-typography-with-rems/)
- [CSS Units Guide - FrontendTools](https://www.frontendtools.tech/blog/css-units-responsive-design-2025)
- [The elements of responsive typography - LogRocket](https://blog.logrocket.com/elements-responsive-typography/)

---

### 4.4 The Fold

**Origin:** In newspaper publishing, "above the fold" referred to the top half of a
broadsheet -- the part visible when the paper was folded on a newsstand. Headlines and
lead stories were placed here for maximum visibility.

**Digital translation:**

"The fold" in web design is the bottom edge of the **initial viewport** -- the point
below which users must scroll to see content.

**Key numbers:**

| Device | Approximate fold position |
|--------|--------------------------|
| Desktop (1920x1080) | ~600-700px from top (after browser chrome) |
| Laptop (1440x900) | ~500-600px from top |
| Tablet portrait | ~700-900px from top |
| Mobile portrait | ~500-700px from top |

**Modern understanding:**

- Users **do scroll** -- the myth that users won't scroll below the fold has been
  debunked. Scroll depth analytics show most users scroll 50-60% of a page.
- However, **attention density** is highest above the fold. The first screenful gets
  disproportionate attention.
- Design should **invite scrolling** rather than cram everything above the fold.
  Visual cues that content continues (cut-off cards, gradient fades, partial images)
  encourage scroll.

**Practical principle:** The fold matters not as a boundary but as a **priority zone**.
Place the most important content and clear visual hierarchy in the first viewport, then
use progressive disclosure for depth.

Sources:
- [Above the Fold - Optimizely](https://www.optimizely.com/optimization-glossary/above-the-fold/)
- [Above the Fold vs. Below the Fold - TheEDigital](https://www.theedigital.com/blog/fold-still-matters)
- [Above the fold - Wikipedia](https://en.wikipedia.org/wiki/Above_the_fold)

---

## 5. Implementation Reference: Concrete Proportional Systems

### 5.1 Margin Ratios

**For a screen-based "page" layout:**

Apply the Van de Graaf / Tschichold proportions adapted for single-screen (non-spread)
context:

```
For a content panel with ratio W:H --

  margin_inner  = W / 9       (or use as left margin on single page)
  margin_top    = H / 9
  margin_outer  = 2 * W / 9   (or use as right margin on single page)
  margin_bottom = 2 * H / 9

For symmetric single-page layout:
  margin_left = margin_right = 1.5 * W / 9  (average of inner and outer)
  margin_top = H / 9
  margin_bottom = 2 * H / 9
```

**Simplified screen-friendly margin ratios:**

| Style | top : right : bottom : left | Notes |
|-------|---------------------------|-------|
| Classical (book) | 1 : 2 : 2 : 1 | Asymmetric, print origin |
| Balanced screen | 1 : 1.5 : 1.5 : 1.5 | Symmetric sides, heavier bottom |
| Equal | 1 : 1 : 1 : 1 | Simple, modern |
| Top-heavy content | 2 : 1 : 1 : 1 | Header-centric |

---

### 5.2 Spacing Scale from Base Unit

**Given a base unit of 8px:**

```rust
// Linear scale (Material Design style)
const SPACE: [f64; 13] = [
    0.0,   // 0
    4.0,   // 1  (half-step)
    8.0,   // 2  (base)
    12.0,  // 3
    16.0,  // 4  (2x base)
    20.0,  // 5
    24.0,  // 6  (3x base)
    32.0,  // 7  (4x base)
    40.0,  // 8  (5x base)
    48.0,  // 9  (6x base)
    64.0,  // 10 (8x base)
    80.0,  // 11 (10x base)
    96.0,  // 12 (12x base)
];

// Geometric scale (Renard R10-inspired, base 8)
const SPACE_GEO: [f64; 10] = [
    2.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0
];
```

**Spacing token assignments:**

```
Inline gap (icon to text):    4px   (0.5x base)
Form field gap:               8px   (1x base)
Card internal padding:        16px  (2x base)
Standard gutter:              24px  (3x base)
Section gap:                  48px  (6x base)
Region gap:                   64px  (8x base)
Page margin (desktop):        64px  (8x base)
```

---

### 5.3 Column Proportions

**12-column grid calculator:**

```
Given:
  viewport_width
  margin (each side)
  gutter_width
  num_columns = 12

content_width = viewport_width - (2 * margin)
column_width  = (content_width - ((num_columns - 1) * gutter_width)) / num_columns
```

**Reference table:**

| Viewport | Margin | Gutter | Content | Column | Columns |
|----------|--------|--------|---------|--------|---------|
| 1440px | 64px | 24px | 1312px | 87.3px | 12 |
| 1280px | 48px | 24px | 1184px | 76.7px | 12 |
| 1024px | 32px | 16px | 960px | 65.3px | 12 |
| 768px | 24px | 16px | 720px | 76.0px | 8 |
| 375px | 16px | 16px | 343px | 77.3px | 4 |

---

### 5.4 Baseline Grid from Font Size and Line-Height

**The calculation:**

```
Given:
  font_size     = 16px
  line_height   = 1.5          (ratio)
  baseline_unit = font_size * line_height = 24px

All vertical spacing should be a multiple of baseline_unit:

  paragraph_spacing   = 1   * baseline_unit = 24px
  heading_margin_top  = 2   * baseline_unit = 48px
  heading_margin_bot  = 0.5 * baseline_unit = 12px
  section_spacing     = 3   * baseline_unit = 72px
  region_spacing      = 4   * baseline_unit = 96px
```

**Fitting larger type sizes to the baseline grid:**

When heading text has a different font size, its line-height must be adjusted to land
on the baseline grid:

```
Given:
  heading_font_size = 32px
  baseline_unit     = 24px

  lines_needed      = ceil(heading_font_size / baseline_unit) = ceil(1.333) = 2
  heading_line_height = lines_needed * baseline_unit = 48px
  effective_ratio     = 48 / 32 = 1.5
```

**Full type scale with baseline-aligned line-heights (base 16px / 24px grid):**

| Role | Font size | Line-height | Lines | Ratio |
|------|-----------|-------------|-------|-------|
| Caption | 12px | 24px | 1 | 2.000 |
| Body | 16px | 24px | 1 | 1.500 |
| Body large | 20px | 24px | 1 | 1.200 |
| H4 | 24px | 48px | 2 | 2.000 |
| H3 | 28px | 48px | 2 | 1.714 |
| H2 | 32px | 48px | 2 | 1.500 |
| H1 | 40px | 48px | 2 | 1.200 |
| Display | 48px | 72px | 3 | 1.500 |
| Hero | 64px | 72px | 3 | 1.125 |

Note: line-height ratios below 1.2 become cramped for multi-line text. For headings
that are typically single-line, ratios down to 1.1 are acceptable.

---

### 5.5 Quick Reference: The Classical Proportions as Code Constants

```rust
/// Classical page proportion systems for layout computation.

/// Van de Graaf / Tschichold canon margins as fractions of page dimension.
/// Works for any page ratio; produces 2:3:4:6 at ratio 2:3.
pub const CANON_INNER_FRAC: f64   = 1.0 / 9.0;  // ~0.1111
pub const CANON_TOP_FRAC: f64     = 1.0 / 9.0;   // ~0.1111
pub const CANON_OUTER_FRAC: f64   = 2.0 / 9.0;   // ~0.2222
pub const CANON_BOTTOM_FRAC: f64  = 2.0 / 9.0;   // ~0.2222

/// Text block occupies this fraction of the page area.
pub const CANON_TEXT_AREA_FRAC: f64 = (1.0 - 1.0/9.0 - 2.0/9.0)
                                    * (1.0 - 1.0/9.0 - 2.0/9.0);
// = (6/9) * (6/9) = 36/81 = 4/9 ~ 0.4444

/// Common page/screen aspect ratios.
pub const RATIO_2_3: f64     = 2.0 / 3.0;    // 0.6667 -- medieval codex
pub const RATIO_3_4: f64     = 3.0 / 4.0;    // 0.7500 -- iPad, traditional monitor
pub const RATIO_GOLDEN: f64  = 1.0 / 1.618;  // 0.6180 -- golden ratio
pub const RATIO_16_9: f64    = 9.0 / 16.0;   // 0.5625 -- widescreen
pub const RATIO_ISO: f64     = 1.0 / 1.4142;  // 0.7071 -- A4/ISO 216 (1:sqrt(2))

/// Typographic scale ratios.
pub const SCALE_MINOR_SECOND: f64   = 1.067;
pub const SCALE_MAJOR_SECOND: f64   = 1.125;
pub const SCALE_MINOR_THIRD: f64    = 1.200;
pub const SCALE_MAJOR_THIRD: f64    = 1.250;
pub const SCALE_PERFECT_FOURTH: f64 = 1.333;
pub const SCALE_AUG_FOURTH: f64     = 1.414;  // sqrt(2)
pub const SCALE_PERFECT_FIFTH: f64  = 1.500;
pub const SCALE_GOLDEN: f64         = 1.618;

/// Spacing base unit (pixels at 1x density).
pub const SPACING_BASE: f64 = 8.0;

/// 8px-based spacing scale.
pub const SPACING_SCALE: [f64; 11] = [
    0.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0, 96.0, 128.0
];
```

---

## Summary of Key Numbers

| Concept | Value |
|---------|-------|
| Classical margin ratio (2:3 page) | 2:3:4:6 (inner:top:outer:bottom) |
| Text block area fraction | 4/9 = 44.4% of page |
| Inner margin fraction | 1/9 of dimension |
| Outer margin fraction | 2/9 of dimension |
| Optimal body line-height | 1.4-1.6x font size |
| Optimal line length | 45-75 characters (~34em) |
| Point to pixel | 1pt = 1.333px (at 96 DPI) |
| Baseline grid unit | font_size * line_height_ratio |
| Spacing base unit | 8px (with 4px half-step) |
| 12-column grid gutter | 16-24px |
| Paragraph spacing | 0.5-1.5x line-height |
| Heading top margin | 1.5-2.5x line-height |
| Desktop reading distance | 20-28 inches |
| Print reading distance | 14-16 inches |
| Screen body text minimum | 16px |
| Renard R5 step factor | ~1.585 (58% per step) |
| Renard R10 step factor | ~1.259 (26% per step) |
