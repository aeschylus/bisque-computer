# Color Theory & Human Attention Modeling

Research compiled for Bisque Computer UI design decisions.

---

## Part I: Color Theory for Typography-First UI

### 1. Opacity-Based Color Hierarchy

A single-hue, varying-opacity approach creates typographic weight without introducing
competing hues. The technique works by modulating the alpha channel of one base color
(typically black or a dark neutral) against a fixed background.

**Recommended opacity scale for black (#000000) on a warm background:**

| Role              | Opacity | Effective use                         |
|-------------------|---------|---------------------------------------|
| Primary text      | 100%    | Headlines, body copy                  |
| Section headers   | 80%     | Panel titles, group labels            |
| Secondary text    | 50%     | Descriptions, supplementary info      |
| Annotations       | 40%     | Timestamps, metadata, captions        |
| Rules / dividers  | 15%     | Hairlines, separator strokes          |
| Ghost / disabled  | 8-10%   | Placeholder text, inactive elements   |

**Why this works better than multiple hues:**

1. **Perceptual monotonicity.** The human visual system processes luminance differences
   faster and more reliably than chromatic differences. A single-hue ramp guarantees
   that every step in the hierarchy maps to a strictly ordered luminance value, which
   the eye parses pre-attentively.

2. **Reduced system complexity.** Transparent shades adapt to different backgrounds
   automatically and maintain consistent contrast ratios. Solid multi-hue systems
   require exponentially more variables -- Eduardo Ferreira documents that solid shade
   systems need separate tokens for every background variant, while opacity systems
   need only one token per hierarchy level.
   ([Source: UX Planet -- Transparent vs. Solid Shades](https://uxplanet.org/designing-color-systems-transparent-vs-solid-shades-9eb841571fdd))

3. **Cognitive coherence.** When all text shares a single hue, the viewer's color
   processing channel is freed for actual data encoding (accent colors, status
   indicators). This aligns with Tufte's principle that color should carry information,
   not decoration (see Section 5).

4. **Hierarchy without distraction.** Erik Kennedy's practical framework recommends
   using color variations (darker/lighter) of the same hue to create hierarchy, noting
   that multiple hues compete for attention and flatten the visual order.
   ([Source: Learn UI Design -- Color in UI Design](https://www.learnui.design/blog/color-in-ui-design-a-practical-framework.html))

**Implementation note:** When compositing opacity-based text over a non-white
background (e.g., bisque #FFE4C4), the effective rendered color will be a blend.
For black at 50% opacity on bisque, the rendered hex is approximately #7F7262.
Always verify contrast ratios on the *composited* color, not the raw opacity value.

---

### 2. Warm Neutral Palettes -- The Science

Warm off-white backgrounds reduce eye strain through three mechanisms:

**A. Reduced short-wavelength (blue) light emission**

LCD and OLED screens emit blue light peaks at ~450nm. Pure white (#FFFFFF) maximizes
blue channel output (B=255). Warm neutrals suppress the blue channel:

| Color        | Hex       | R   | G   | B   | Blue reduction vs white |
|--------------|-----------|-----|-----|-----|------------------------|
| Bisque       | #FFE4C4   | 255 | 228 | 196 | 23%                    |
| Antique White| #FAEBD7   | 250 | 235 | 215 | 16%                    |
| Linen        | #FAF0E6   | 250 | 240 | 230 | 10%                    |
| Old Lace     | #FDF5E6   | 253 | 245 | 230 | 10%                    |
| Papaya Whip  | #FFEFD5   | 255 | 239 | 213 | 16%                    |

Blue light suppression matters because melanopsin-containing retinal ganglion cells
are maximally sensitive to ~480nm light, and chronic exposure contributes to digital
eye strain and circadian disruption.
([Source: Iris Tech -- Best Background Color to Reduce Eye Strain](https://iristech.co/best-background-color-to-reduce-eye-strain/))

**B. Reduced luminance contrast with ambient environment**

Pure white backgrounds at typical screen brightness (300-400 cd/m2) create a
significant luminance differential with the surrounding environment, forcing the
iris to continuously adjust. Warm neutrals lower the screen-to-surround ratio,
reducing pupil oscillation and accommodative fatigue.

**C. Warmth perception and cognitive comfort**

Research on color temperature associations shows that warm hues (reds, yellows,
oranges) evoke associations with natural light, parchment, and physical warmth.
This psychological warmth reduces perceived cognitive effort during sustained reading.
([Source: Color With Leo -- What color is least damaging to eyes?](https://www.colorwithleo.com/what-color-is-least-damaging-to-eyes/))

**Contrast ratios with black text (#000000):**

| Background     | Hex       | Relative Luminance | Contrast with #000 |
|----------------|-----------|-------------------|---------------------|
| White          | #FFFFFF   | 1.0000            | 21.0:1              |
| Old Lace       | #FDF5E6   | 0.9062            | 19.2:1              |
| Linen          | #FAF0E6   | 0.8780            | 18.6:1              |
| Papaya Whip    | #FFEFD5   | 0.8696            | 18.4:1              |
| Antique White  | #FAEBD7   | 0.8357            | 17.8:1              |
| Bisque         | #FFE4C4   | 0.7895            | 16.8:1              |

All of these exceed WCAG AAA requirements (7:1) by a wide margin. Bisque is the
warmest option while still providing excellent contrast for black text.

**Recommended for Bisque Computer:** #FFE4C4 (bisque) as primary background, with
#FAEBD7 (antique white) as an alternate surface color for cards or inset panels.

---

### 3. Solarized and Principled Palette Design

Ethan Schoonover's Solarized (2011) is the canonical example of a perceptually
principled color palette. Its design methodology provides a model for any
typographic-first UI.
([Source: Ethan Schoonover -- Solarized](https://ethanschoonover.com/solarized/))

**Structure:** 16 colors = 8 monotones + 8 accent colors.

**Monotone ramp (CIELAB L* values):**

| Name    | Hex       | L*  | Role (dark mode)     | Role (light mode)    |
|---------|-----------|-----|----------------------|----------------------|
| base03  | #002b36   | 15  | Background           | --                   |
| base02  | #073642   | 20  | Background highlight | --                   |
| base01  | #586e75   | 45  | Secondary content    | Emphasized content   |
| base00  | #657b83   | 50  | Primary content      | --                   |
| base0   | #839496   | 60  | --                   | Primary content      |
| base1   | #93a1a1   | 65  | Emphasized content   | Secondary content    |
| base2   | #eee8d5   | 92  | --                   | Background highlight |
| base3   | #fdf6e3   | 97  | --                   | Background           |

**Key design principles:**

1. **Symmetric CIELAB lightness.** The L* gaps between monotones are symmetric:
   15-20-45-50 // 60-65-92-97. The delta between background and highlight is 5 in
   both modes. The delta between primary and secondary content is also 5. Switching
   between dark and light mode preserves identical perceptual contrast.

2. **Fixed a*b* for monotones.** All eight monotones share approximately the same
   a*b* chromaticity (around a=-7 to -12, b=-3 to -12 for cool tones, and a=0, b=10
   for the warm base2/base3). This means the monotone ramp is perceived as a single
   color "family" at different brightnesses.

3. **Accent colors at uniform L*=50 or L*=60.** All accent colors share one of two
   CIELAB lightness values, ensuring they have equal visual weight when used for syntax
   highlighting or UI elements:

| Accent    | Hex       | L*  | a*  | b*   |
|-----------|-----------|-----|-----|------|
| Yellow    | #b58900   | 60  | 10  | 65   |
| Orange    | #cb4b16   | 50  | 50  | 55   |
| Red       | #dc322f   | 50  | 65  | 45   |
| Magenta   | #d33682   | 50  | 65  | -5   |
| Violet    | #6c71c4   | 50  | 15  | -45  |
| Blue      | #268bd2   | 55  | -10 | -45  |
| Cyan      | #2aa198   | 60  | -35 | -5   |
| Green     | #859900   | 60  | -20 | 65   |

4. **Color wheel distribution.** The 8 accents span the full 360-degree hue circle
   with roughly even angular spacing (45-degree intervals), maximizing perceptual
   distinguishability.

**Applicability to Bisque Computer:** The Solarized approach validates the design
decision of using a warm background (base3 = #fdf6e3, L*=97, is close to linen).
For a bisque-based palette, accent colors should be chosen at a consistent CIELAB
lightness value to ensure equal visual weight.
([Source: Wikipedia -- Solarized](https://en.wikipedia.org/wiki/Solarized))

---

### 4. Contrast Ratios -- WCAG Compliance

**WCAG 2.1 requirements:**

| Level | Normal text (<18pt / <14pt bold) | Large text (>=18pt / >=14pt bold) |
|-------|----------------------------------|-----------------------------------|
| AA    | 4.5:1                            | 3:1                               |
| AAA   | 7:1                              | 4.5:1                             |

Non-text UI components and graphical objects require 3:1 minimum contrast.
([Source: W3C -- Understanding SC 1.4.3](https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum.html))

**Why 4.5:1?** This threshold compensates for contrast sensitivity loss equivalent
to approximately 20/40 vision, which is the typical acuity of people at roughly age
80. It ensures legibility across a wide range of visual abilities.
([Source: W3C -- Techniques G18](https://www.w3.org/WAI/WCAG21/Techniques/general/G18))

**The contrast ratio formula:**

```
ContrastRatio = (L1 + 0.05) / (L2 + 0.05)
```

Where L1 = relative luminance of the lighter color, L2 = relative luminance of the
darker color.

**Calculating relative luminance (step by step):**

1. Normalize each 8-bit channel: `RsRGB = R / 255`

2. Linearize (remove gamma):
   ```
   if RsRGB <= 0.04045:
       R_lin = RsRGB / 12.92
   else:
       R_lin = ((RsRGB + 0.055) / 1.055) ^ 2.4
   ```
   (Apply identically to G and B.)

3. Weighted sum:
   ```
   L = 0.2126 * R_lin + 0.7152 * G_lin + 0.0722 * B_lin
   ```

The weights reflect the human eye's spectral sensitivity: green contributes ~72%
of perceived brightness, red ~21%, blue ~7%.
([Source: W3C -- Relative Luminance](https://www.w3.org/WAI/GL/wiki/Relative_luminance))

**Worked example -- Black on Bisque (#000000 on #FFE4C4):**

- Bisque: R=255, G=228, B=196
  - R_lin = 1.0, G_lin = 0.7874, B_lin = 0.5559
  - L_bisque = 0.2126(1.0) + 0.7152(0.7874) + 0.0722(0.5559) = 0.2126 + 0.5632 + 0.0401 = 0.8159
- Black: L_black = 0.0
- Contrast = (0.8159 + 0.05) / (0.0 + 0.05) = 0.8659 / 0.05 = **17.3:1**

This exceeds AAA requirements. Black text on bisque is robustly accessible.

**Opacity-adjusted contrast check example -- 50% black on bisque:**

- Composited color: R=128, G=114, B=98 (approximately #80725E)
- R_lin = 0.2158, G_lin = 0.1690, B_lin = 0.1107
- L_composite = 0.2126(0.2158) + 0.7152(0.1690) + 0.0722(0.1107) = 0.0459 + 0.1209 + 0.0080 = 0.1748
- Contrast with bisque = (0.8159 + 0.05) / (0.1748 + 0.05) = 0.8659 / 0.2248 = **3.85:1**

This passes for large text (3:1) but fails for normal body text (4.5:1). At 50%
opacity, text should be at least 18pt or 14pt bold. For normal-sized secondary text,
use 60-65% opacity minimum.

**Tools for checking:**
- [WebAIM Contrast Checker](https://webaim.org/resources/contrastchecker/)
- [Siege Media Contrast Ratio](https://www.siegemedia.com/contrast-ratio)
- [Accessible Web Color Contrast Checker](https://accessibleweb.com/color-contrast-checker/)

---

### 5. Color as Information, Not Decoration

Edward Tufte's first principle of color use in "Envisioning Information" (1990):

> **"Above all, do no harm."**

Tufte's color principles, paraphrased from Chapter 5:

1. **Color should encode data.** Every color in a visualization or interface should
   correspond to a meaningful variable. If a color does not encode information, it is
   noise.

2. **The smallest effective difference.** Use the minimum color change needed to
   communicate a distinction. Loud, saturated colors over large areas produce
   "unbearable effects" -- reserve strong colors for small areas at the extremes of
   a data range.

3. **Limit the palette.** The eye can reliably distinguish 20-30 colors in context.
   Beyond that, returns are not merely diminishing but negative -- more colors produce
   more confusion, not more clarity.

4. **Use muted backgrounds.** "Pure, bright or very strong colors have loud,
   unbearable effects when they stand unrelieved over large areas adjacent to each
   other, but extraordinary effects can be achieved when they are used sparingly on
   or between dull background tones."

5. **Redundant encoding.** Because color perception varies among viewers and across
   devices, always provide a second channel (shape, position, label) to reinforce
   color-encoded information.

6. **Data-ink ratio.** Maximize the proportion of "ink" (pixels) devoted to data.
   Decorative color (gradients, fills, borders that encode nothing) reduces the
   data-ink ratio.
   ([Source: Tufte -- Envisioning Information Ch.5](https://blogs.ischool.berkeley.edu/i247s12/files/2012/02/Chapter-5-Envisioning-Information-Tufte.pdf))
   ([Source: GeeksforGeeks -- Mastering Tufte's Principles](https://www.geeksforgeeks.org/data-visualization/mastering-tuftes-data-visualization-principles/))

**Implementable rule for Bisque Computer:** Reserve chromatic color exclusively for
actionable or stateful elements: links, active selections, errors, voice-activity
indicators. All structural and typographic elements use the black-opacity ramp on
bisque. This ensures that when color appears, the user's pre-attentive system
immediately registers it as meaningful.

---

## Part II: Human Attention Modeling

### 1. Pre-Attentive Processing

Pre-attentive processing occurs in the earliest stages of visual perception, within
the first 200-250 milliseconds of exposure, before conscious attention is directed.
It is massively parallel -- the entire visual field is processed simultaneously.
([Source: Healey -- Perception in Visualization](https://www.csc2.ncsu.edu/faculty/healey/PP/))

**The four categories of pre-attentive features (Healey & Ware):**

**Form:**
- Line orientation
- Line length
- Line width
- Line collinearity
- Size
- Curvature
- Spatial grouping
- Blur
- Added marks
- Numerosity
- Enclosure

**Color:**
- Hue
- Intensity (luminance)

**Motion:**
- Flicker
- Direction of movement
- Velocity

**Spatial position:**
- 2D position
- Stereoscopic depth
- Convex/concave shape from shading

**Critical constraints:**

1. **Conjunction failure.** Pre-attentive detection works for *individual* features
   but fails for conjunctions. A red circle among blue circles and red squares pops
   out. But a red circle among red squares and blue circles does not -- the visual
   system cannot pre-attentively combine "red" AND "circle."

2. **Asymmetry.** Some searches are asymmetric: finding a tilted line among vertical
   lines is faster than finding a vertical line among tilted lines. Designers should
   use the "pop-out" direction -- make the target the anomaly.

3. **Interference.** Using more than ~4 pre-attentive features simultaneously causes
   interference and reduces detection accuracy.
   ([Source: IxDF -- Preattentive Visual Properties](https://www.interaction-design.org/literature/article/preattentive-visual-properties-and-how-to-use-them-in-information-visualization))

**Design implications for Bisque Computer:**

- Use a maximum of 3-4 visual channels simultaneously (e.g., position + size + one
  hue dimension + opacity).
- Make the most important UI change the *only* anomalous feature in its region.
  A single accent-colored element on a monochrome field is pre-attentively detectable.
  The same element among other colored elements is not.

---

### 2. Visual Saliency -- The Itti-Koch Model

Laurent Itti and Christof Koch (1998) proposed the foundational computational model
of bottom-up visual attention:
([Source: Itti, Koch & Niebur -- IEEE TPAMI 1998](https://www.researchgate.net/publication/3192913_A_Model_of_Saliency-based_Visual_Attention_for_Rapid_Scene_Analysis))

**Model architecture:**

1. The visual input is decomposed into three parallel feature channels:
   - **Intensity** (luminance contrast)
   - **Color** (red-green and blue-yellow opponency)
   - **Orientation** (0, 45, 90, 135 degrees via Gabor filters)

2. Each channel is processed at multiple spatial scales (center-surround differences)
   to detect local contrast.

3. The three conspicuity maps are normalized and combined into a single
   **saliency map** -- a 2D representation where each point's value indicates how
   visually conspicuous that location is.

4. A winner-take-all network selects the most salient location. After attending to
   it, **inhibition of return** suppresses that location, and attention shifts to
   the next most salient point.

**Key principles for UI design:**

| Factor       | Effect                                                        |
|--------------|---------------------------------------------------------------|
| Contrast     | High local contrast (luminance or color) attracts fixation    |
| Isolation    | A single distinct element in a uniform field is maximally salient |
| Anomaly      | Deviation from a pattern attracts attention (oddball effect)  |
| Scale        | Large features attract before small ones at equivalent contrast |
| Orientation  | Oblique lines are more salient than cardinal (horizontal/vertical) |

**Implementable rules:**

- Place the most important interactive element where it creates the highest local
  contrast against its surroundings.
- Use isolation: a single colored element on a monochrome background is maximally
  salient. Two colored elements split saliency.
- Suppress saliency for secondary elements: reduce their contrast (lower opacity)
  so the winner-take-all mechanism converges on the primary element.

---

### 3. Fitts's Law

Paul Fitts (1954) established the fundamental relationship between motor control
and target acquisition:

```
MT = a + b * log2(2D / W)
```

Where:
- **MT** = movement time (ms)
- **D** = distance from starting position to target center
- **W** = width of the target along the axis of motion
- **a** = intercept (device start/stop time, typically 50-100ms)
- **b** = slope (inherent speed of the device, typically 100-150ms/bit for mouse)
- **log2(2D/W)** = Index of Difficulty (ID), measured in bits

The relationship is *logarithmic* -- doubling the distance does not double the time.
([Source: IxDF -- Fitts's Law](https://www.interaction-design.org/literature/article/fitts-s-law-the-importance-of-size-and-distance-in-ui-design))
([Source: NN/g -- Fitts's Law and Its Applications in UX](https://www.nngroup.com/articles/fitts-law/))

**Numeric examples:**

| D (px) | W (px) | ID (bits) | Approx MT (ms) |
|--------|--------|-----------|-----------------|
| 100    | 50     | 2.0       | 250             |
| 200    | 50     | 3.0       | 350             |
| 400    | 50     | 4.0       | 450             |
| 100    | 100    | 1.0       | 150             |
| 100    | 25     | 3.0       | 350             |

(Assuming a=50ms, b=100ms/bit)

**Design implications:**

1. **Minimum touch target:** 44x44pt (Apple HIG), 48x48dp (Material Design). These
   sizes keep ID below ~3 bits for typical finger movements.

2. **Padding counts.** Expanding the clickable/tappable area beyond the visible
   element boundary reduces effective ID. A 12px text link with 8px padding on each
   side has an effective W of 28px, not 12px.

3. **Edge and corner advantage.** Screen edges act as infinite-width targets (the
   cursor cannot overshoot). Placing important controls at screen edges effectively
   sets W = infinity, making ID approach 0. This is why macOS menu bars at the top
   edge are fast to acquire.

4. **Proximity grouping.** Related actions should be near each other to minimize D.
   A confirmation button should be near the element that triggered the dialog.

---

### 4. Gestalt Principles

The Gestalt principles of perceptual organization, established by Wertheimer, Koffka,
and Kohler in the 1920s, explain how the visual system groups elements into coherent
structures -- enabling layout *without* explicit boxes or borders.
([Source: IxDF -- Gestalt Principles](https://www.interaction-design.org/literature/topics/gestalt-principles))

**The six principles and their UI applications:**

**Proximity** -- Elements near each other are perceived as a group.
- *Numeric guideline:* Items within 1.5x their own size of each other group
  perceptually. Items separated by 3x+ their size are perceived as separate.
- *UI application:* Use whitespace instead of borders to delineate sections. A
  24px gap between groups and 8px gap within groups creates clear hierarchy without
  any visual chrome.

**Similarity** -- Elements sharing visual properties (color, shape, size, orientation)
are perceived as related.
- *UI application:* All interactive elements share one accent color. All metadata
  shares one opacity level. Consistency of treatment implies functional equivalence.

**Continuity** -- The eye follows smooth paths and lines.
- *UI application:* Align elements along a grid axis. The eye follows the alignment
  edge as a continuous path, connecting visually separated elements into a coherent
  flow.

**Closure** -- The mind completes incomplete shapes.
- *UI application:* Partial borders (a bottom-only rule, a left-side accent bar) are
  sufficient to imply a container. Full rectangular borders are rarely necessary.

**Figure/Ground** -- The visual system separates foreground objects from background.
- *UI application:* Subtle background tone shifts (bisque to antique white) create
  figure/ground separation without borders. Elevation (shadow) is another cue but
  carries more visual weight.

**Common Fate** -- Elements moving in the same direction are perceived as grouped.
- *UI application:* Scroll regions, animated transitions. Elements that move together
  during a layout shift are understood as a unit.

**Implementable rule for Bisque Computer:** Rely on proximity and alignment as the
primary grouping mechanisms. Reserve borders and background shifts for major
structural divisions (pane boundaries). Within a pane, whitespace alone should
communicate grouping.

---

### 5. F-Pattern and Z-Pattern Scanning

Eye-tracking research by Jakob Nielsen (Nielsen Norman Group, 2006) identified
dominant scanning patterns on content-heavy pages:
([Source: NN/g -- F-Shaped Pattern](https://www.nngroup.com/articles/f-shaped-pattern-reading-web-content-discovered/))

**F-Pattern (text-heavy content):**

1. Horizontal sweep across the top ~1/3 of the content area
2. A shorter horizontal sweep slightly below
3. Vertical scan down the left edge

This produces a heatmap shaped like the letter F. The pattern means:
- The first two words of every line receive the most fixations
- Content placement is critical: the most important information goes top-left
- Left-aligned text outperforms centered text for scannability

**Z-Pattern (sparse/visual layouts):**

1. Horizontal sweep across the top (logo, navigation)
2. Diagonal sweep to the bottom-left
3. Horizontal sweep across the bottom (CTA, footer actions)

**Four scanning patterns identified by NN/g:**

| Pattern      | Trigger                      | Implication                          |
|--------------|------------------------------|--------------------------------------|
| F-pattern    | Dense text, lists            | Front-load key words on each line    |
| Spotted      | Scanning for specific item   | Use visual markers (bold, color)     |
| Layer-cake   | Headings with body text      | Strong heading hierarchy works       |
| Commitment   | Motivated reader             | Full reading occurs -- optimize line length |

([Source: NN/g -- Text Scanning Patterns](https://www.nngroup.com/articles/text-scanning-patterns-eyetracking/))

**Implementable rules:**

- Place the most critical information in the top-left quadrant.
- Use a strong left edge (consistent left alignment) to support the vertical scan.
- Keep line lengths to 45-75 characters to support the commitment pattern without
  requiring excessive horizontal eye movement.
- Use a layer-cake structure: visually distinct headings separated by body text.
  This is the most accessible scanning pattern because it works for both scanning
  and reading.

---

### 6. Cognitive Load Theory

**Miller's Law (1956):** Working memory holds 7 +/- 2 chunks of information
simultaneously. Modern research (Cowan, 2001) revises this to approximately **4 chunks**
for novel information.
([Source: Laws of UX -- Miller's Law](https://lawsofux.com/millers-law/))

**Chunking:** Information grouped into meaningful units occupies fewer working memory
slots. "FBI-CIA-NASA" is 3 chunks; "FBICIANASA" is 9 characters competing for
individual slots. Visual proximity and similarity (Gestalt) create perceptual chunks
automatically.
([Source: Instructional Design Junction -- 7 +/- 2 Rule](https://instructionaldesignjunction.com/2021/08/23/george-a-millers-7-plus-or-minus-2-rule-and-simon-and-chases-chunking-principle/))

**Cognitive Load Theory (Sweller, 1988):**

Three types of cognitive load:

| Type        | Definition                                         | Design response                |
|-------------|----------------------------------------------------|--------------------------------|
| Intrinsic   | Complexity inherent to the task                    | Cannot be reduced by design    |
| Extraneous  | Load imposed by poor presentation                  | Minimize through design        |
| Germane     | Load devoted to learning/schema building           | Support and encourage          |

**Progressive disclosure:** Show only essential information at each step. Details
are available on demand. This reduces extraneous load by limiting visible elements
to those relevant to the current task.

**Whitespace as a cognitive tool:**

Whitespace is not "empty space" -- it is an active element that:

1. **Reduces visual noise.** Fewer elements competing for attention means lower
   extraneous load.
2. **Creates chunks.** Spatial separation groups related items, reducing the number
   of perceived units from N items to N/k groups.
3. **Provides rest points.** The eye needs periodic "landings" in low-information
   areas to consolidate processing of adjacent high-information areas.
4. **Signals hierarchy.** Larger gaps indicate larger conceptual separations.

**Numeric guidelines for whitespace:**

| Relationship              | Recommended spacing       |
|---------------------------|---------------------------|
| Between related items     | 4-8px (0.25-0.5em)        |
| Between groups            | 16-24px (1-1.5em)         |
| Between sections          | 32-48px (2-3em)           |
| Between major regions     | 64-96px (4-6em)           |
| Page margins              | >= 5% of viewport width   |

---

### 7. Colin Ware -- Information Visualization: Perception for Design

Colin Ware's "Information Visualization: Perception for Design" (now in its 4th
edition, 2021) provides the most comprehensive bridge between vision science and
practical design. The book contains 160+ explicit design guidelines.
([Source: Scholars UNH -- Ware](https://scholars.unh.edu/ccom/127/))
([Source: Amazon -- Information Visualization](https://www.amazon.com/Information-Visualization-Perception-Interactive-Technologies/dp/0123814642))

**Key principles:**

**1. The visual system as a pipeline.**

Ware models visual processing as a three-stage pipeline:

| Stage | Process                    | Speed         | Design leverage                    |
|-------|----------------------------|---------------|------------------------------------|
| 1     | Parallel feature extraction| <200ms        | Pre-attentive: color, form, motion |
| 2     | Pattern perception         | 200ms-2s      | Gestalt grouping, texture, contour |
| 3     | Sequential goal-directed   | 2s+           | Reading, comparison, reasoning     |

Design for Stage 1 first. If the most important information is encoded in Stage 1
features, users perceive it before they consciously look for it.

**2. Magnitude vs. identity channels.**

Visual properties divide into two types:

- **Magnitude channels** (how much?): Position, length, area, luminance, saturation.
  These support *ordered* data -- the eye can judge "more" or "less."

- **Identity channels** (what kind?): Hue, shape, texture pattern. These support
  *categorical* data -- the eye distinguishes "different" but not "more."

*Rule:* Never use a magnitude channel for categorical data (e.g., size to indicate
type) or an identity channel for quantitative data (e.g., hue to indicate amount).

**Channel effectiveness ranking (for quantitative data):**

1. Position along a common scale (most accurate)
2. Position along non-aligned scales
3. Length
4. Angle / slope
5. Area
6. Luminance / saturation
7. Color hue (least accurate for quantities)

**3. The pop-out principle.**

A visual feature that differs from all surrounding features in exactly one Stage 1
dimension is detected pre-attentively. This is the fastest possible route to user
attention. Ware emphasizes that pop-out is *destroyed* by heterogeneity in the
surrounding field -- if the context is already multi-colored, an additional color
does not pop out.

**4. Perceptual layers through color.**

Ware describes using luminance contrast to create perceptual "layers" that the visual
system separates automatically. High-contrast elements (dark on light) form the
foreground layer; low-contrast elements (gray on light) recede to a background layer.
This is the perceptual basis for the opacity hierarchy described in Part I, Section 1.

**5. Gray as the universal background.**

Ware notes that medium gray (or neutral warm tones) is the optimal background for
color-coded information because it minimizes simultaneous contrast effects -- colors
appear most "true" against a neutral surround. A warm neutral like bisque functions
similarly, with the additional benefit of reduced blue light.

---

## Appendix: Implementable Rules Summary

### Color Rules

| # | Rule | Source |
|---|------|--------|
| C1 | Use a single-hue opacity ramp for all typographic hierarchy | Kennedy, Ferreira |
| C2 | Background: bisque #FFE4C4 (L_rel=0.82, 23% blue reduction) | Eye strain research |
| C3 | Minimum 60% opacity for body text on bisque (contrast >= 4.5:1) | WCAG 2.1 AA |
| C4 | Reserve chromatic color for actionable/stateful elements only | Tufte |
| C5 | Maximum 5-7 chromatic colors in the full palette | Tufte, Miller |
| C6 | All accent colors at equal CIELAB L* value | Schoonover/Solarized |
| C7 | Always provide a non-color redundant encoding | Tufte, WCAG |
| C8 | Verify contrast on composited (flattened) colors, not raw opacity | WCAG formula |

### Attention Rules

| # | Rule | Source |
|---|------|--------|
| A1 | Encode the most important state change in a single pre-attentive feature | Healey, Ware |
| A2 | Maximum 3-4 simultaneous visual encoding channels | Healey |
| A3 | Use isolation (single anomaly in uniform field) for maximum saliency | Itti-Koch |
| A4 | Minimum touch target: 44x44pt; extend hit areas with padding | Fitts, Apple HIG |
| A5 | Group by proximity and alignment, not by boxes | Gestalt |
| A6 | Critical content in top-left quadrant; strong left edge | NN/g F-pattern |
| A7 | Limit visible items to ~4 groups (Cowan's revised chunk limit) | Miller, Cowan |
| A8 | Whitespace between groups >= 2x whitespace within groups | Gestalt proximity |
| A9 | Line length: 45-75 characters for sustained reading | NN/g |
| A10 | Progressive disclosure: show details on demand, not by default | Sweller CLT |

---

## Sources

- [Erik Kennedy -- Color in UI Design: A Practical Framework](https://www.learnui.design/blog/color-in-ui-design-a-practical-framework.html)
- [Eduardo Ferreira -- Designing Color Systems: Transparent vs. Solid Shades (UX Planet)](https://uxplanet.org/designing-color-systems-transparent-vs-solid-shades-9eb841571fdd)
- [Iris Tech -- Best Background Color to Reduce Eye Strain](https://iristech.co/best-background-color-to-reduce-eye-strain/)
- [Color With Leo -- What color is least damaging to eyes?](https://www.colorwithleo.com/what-color-is-least-damaging-to-eyes/)
- [GLARminY -- Anti Computer Eye Strain Colors](https://glarminy.com/2016/03/29/anti-computer-eye-strain-colors/)
- [PMC -- Effect of Ambient Illumination and Text Color on Visual Fatigue](https://pmc.ncbi.nlm.nih.gov/articles/PMC11175232/)
- [Ethan Schoonover -- Solarized](https://ethanschoonover.com/solarized/)
- [Wikipedia -- Solarized](https://en.wikipedia.org/wiki/Solarized)
- [GitHub -- altercation/solarized](https://github.com/altercation/solarized)
- [W3C -- Understanding SC 1.4.3: Contrast (Minimum)](https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum.html)
- [W3C -- Techniques G18](https://www.w3.org/WAI/WCAG21/Techniques/general/G18)
- [W3C -- Relative Luminance](https://www.w3.org/WAI/GL/wiki/Relative_luminance)
- [WebAIM -- Contrast and Color Accessibility](https://webaim.org/articles/contrast/)
- [WebAIM -- Contrast Checker](https://webaim.org/resources/contrastchecker/)
- [Matthew Hallonbacka -- How does the WCAG contrast formula work?](https://mallonbacka.com/blog/2023/03/wcag-contrast-formula/)
- [TestParty -- WCAG 4.5:1 Contrast Ratio Guide](https://testparty.ai/blog/wcag-contrast-ratio-guide-2025)
- [Tufte -- Envisioning Information, Chapter 5 (PDF)](https://blogs.ischool.berkeley.edu/i247s12/files/2012/02/Chapter-5-Envisioning-Information-Tufte.pdf)
- [Edward Tufte -- Official Site](https://www.edwardtufte.com/)
- [GeeksforGeeks -- Mastering Tufte's Data Visualization Principles](https://www.geeksforgeeks.org/data-visualization/mastering-tuftes-data-visualization-principles/)
- [CMS633 MIT -- Tufte Color and Information Commentary](https://cms633.github.io/Fall-2018/commentary/edward-tufte-color-and-information.html)
- [Christopher Healey -- Perception in Visualization (NC State)](https://www.csc2.ncsu.edu/faculty/healey/PP/)
- [IxDF -- Preattentive Visual Properties](https://www.interaction-design.org/literature/article/preattentive-visual-properties-and-how-to-use-them-in-information-visualization)
- [IxDF -- What are Preattentive Visual Properties?](https://www.interaction-design.org/literature/topics/preattentive-visual-properties)
- [Itti, Koch & Niebur -- A Model of Saliency-based Visual Attention (IEEE TPAMI 1998)](https://www.researchgate.net/publication/3192913_A_Model_of_Saliency-based_Visual_Attention_for_Rapid_Scene_Analysis)
- [Itti & Koch -- A saliency-based search mechanism (Vision Research 2000)](https://pubmed.ncbi.nlm.nih.gov/10788654/)
- [IxDF -- Fitts's Law: Size and Distance in UI Design](https://www.interaction-design.org/literature/article/fitts-s-law-the-importance-of-size-and-distance-in-ui-design)
- [NN/g -- Fitts's Law and Its Applications in UX](https://www.nngroup.com/articles/fitts-law/)
- [Laws of UX -- Fitts's Law](https://lawsofux.com/fittss-law/)
- [Figma -- Fitts' Law](https://www.figma.com/resource-library/fitts-law/)
- [IxDF -- Gestalt Principles](https://www.interaction-design.org/literature/topics/gestalt-principles)
- [Toptal -- Exploring the Gestalt Principles of Design](https://www.toptal.com/designers/ui/gestalt-principles-of-design)
- [UserTesting -- 7 Gestalt Principles](https://www.usertesting.com/blog/gestalt-principles)
- [NN/g -- F-Shaped Pattern Reading Web Content](https://www.nngroup.com/articles/f-shaped-pattern-reading-web-content-discovered/)
- [NN/g -- Text Scanning Patterns: Eyetracking Evidence](https://www.nngroup.com/articles/text-scanning-patterns-eyetracking/)
- [NN/g -- F-Shaped Pattern Still Relevant on Mobile](https://www.nngroup.com/articles/f-shaped-pattern-reading-web-content/)
- [Laws of UX -- Miller's Law](https://lawsofux.com/millers-law/)
- [Instructional Design Junction -- Miller's 7 +/- 2 and Chunking](https://instructionaldesignjunction.com/2021/08/23/george-a-millers-7-plus-or-minus-2-rule-and-simon-and-chases-chunking-principle/)
- [IJRASET -- Reducing Cognitive Load in UI Design](https://www.ijraset.com/research-paper/reducing-cognitive-load-in-ui-design)
- [Colin Ware -- Information Visualization: Perception for Design (UNH Scholars)](https://scholars.unh.edu/ccom/127/)
- [Colin Ware -- Visual Thinking for Design (UNH Scholars)](https://scholars.unh.edu/ccom/145/)
- [Amazon -- Information Visualization: Perception for Design, 3rd Ed.](https://www.amazon.com/Information-Visualization-Perception-Interactive-Technologies/dp/0123814642)
