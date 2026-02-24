# Bisque Computer

A native macOS dashboard client built with Rust (vello/winit).

## Post-Change Requirement

**IMPORTANT:** After ANY change is committed and pushed, you MUST send the user instructions to re-download and open the latest build. Include this in your message:

### For Apple Silicon (M1/M2/M3/M4):
```
curl -L https://github.com/aeschylus/bisque-computer/releases/latest/download/BisqueComputer-aarch64-apple-darwin.tar.gz -o /tmp/BisqueComputer.tar.gz && tar xzf /tmp/BisqueComputer.tar.gz -C /tmp && open /tmp/BisqueComputer.app
```

### For Intel Mac:
```
curl -L https://github.com/aeschylus/bisque-computer/releases/latest/download/BisqueComputer-x86_64-apple-darwin.tar.gz -o /tmp/BisqueComputer.tar.gz && tar xzf /tmp/BisqueComputer.tar.gz -C /tmp && open /tmp/BisqueComputer.app
```

**Note:** A new release must be tagged for these downloads to reflect the latest changes. After pushing to main, tag a new version and push the tag to trigger the release workflow:
```
git tag vX.Y.Z && git push origin vX.Y.Z
```

## Design System

**MANDATORY:** All UI work MUST follow the Bisque Design System defined in `skills/design-system.md`. Read it before making any visual changes. The Rust implementation lives in `src/design.rs` — use its tokens and functions, do not hardcode colors, sizes, or spacing.

### Core constraints (always apply these)

1. **No boxes.** No cards, panels, filled rectangles, rounded rects, progress bars, colored badges, shadows, or gradients. Structure through typography, whitespace, and thin rules only.
2. **Ink hierarchy.** Black at varying opacities on bisque: 100% (headings), 80% (sections), 70% (body), 50% (secondary), 40% (annotations), 15% (rules), 8% (ghost/disabled). Use `design::INK_*` tokens.
3. **Type scale.** Perfect Fourth (1.333) at 18px base. Use `design::type_size(step)` or the named constants (`TYPE_SM` through `TYPE_4XL`). Optima for text, Monaco for code.
4. **Baseline grid.** 28px unit. All vertical spacing as multiples: `SPACE_QUARTER` (7), `SPACE_HALF` (14), `SPACE_ONE` (28), `SPACE_TWO` (56), `SPACE_THREE` (84).
5. **Sections.** Title in `TYPE_XL` + `INK_SECTION` → 0.5px rule in `INK_RULE` → content below with `SPACE_HALF` gap. Between sections: `SPACE_TWO`.
6. **Color encodes data only.** Chromatic color reserved for actionable/stateful elements (links, errors, voice indicators). Never decorative.
7. **Strong left edge.** Consistent left margin alignment. F-pattern scannability.

### Key files
- `skills/design-system.md` — Full specification with principles P1-P10
- `src/design.rs` — Rust tokens, type scale, spacing, contrast, animation utilities
- `skills/sources/` — Research backing (typography, color theory, layout, visualization pioneers, Rust crates)

### Design inspirations
Tufte, Bringhurst, Tschichold, Muller-Brockmann, Wilkinson (Grammar of Graphics), Mike Bostock, Nadieh Bremer, Santiago Ortiz, fullyparsed.com. Think: printed monograph, not software UI.
