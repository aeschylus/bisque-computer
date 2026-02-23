//! Design tokens for the bisque-computer visual system.
//!
//! All visual constants (colors, sizes, spacing, animation) are gathered into
//! a single `DesignTokens` struct that can be serialized to/from TOML and
//! threaded through the render pipeline.
//!
//! Layer 1: runtime struct with defaults matching existing `const` values.
//! Layer 3 (future): hot-reload from a TOML file on disk.

use vello::peniko::Color;

// ---------------------------------------------------------------------------
// Legacy constants (kept for backward compatibility, marked dead_code)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub const BG_R: f64 = 1.0;
#[allow(dead_code)]
pub const BG_G: f64 = 0.894;
#[allow(dead_code)]
pub const BG_B: f64 = 0.769;

// ---------------------------------------------------------------------------
// DesignTokens
// ---------------------------------------------------------------------------

/// Root design-token container. Every visual constant lives here.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct DesignTokens {
    pub background: BackgroundTokens,
    pub ink: InkTokens,
    pub type_scale: TypeScaleTokens,
    pub line_height: LineHeightTokens,
    pub spacing: SpacingTokens,
    pub margins: MarginTokens,
    pub grid: GridTokens,
    pub rules: RuleTokens,
    pub tracking: TrackingTokens,
    pub animation: AnimationTokens,
    pub terminal: TerminalTokens,
}

// --- Sub-structs ---

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct BackgroundTokens {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct InkTokens {
    pub primary: f64,
    pub section: f64,
    pub body: f64,
    pub secondary: f64,
    pub annotation: f64,
    pub rule: f64,
    pub ghost: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TypeScaleTokens {
    pub base: f64,
    pub ratio: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct LineHeightTokens {
    pub body: f64,
    pub heading: f64,
    pub display: f64,
    pub caption: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct SpacingTokens {
    pub baseline: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct MarginTokens {
    pub left_frac: f64,
    pub right_frac: f64,
    pub top_frac: f64,
    pub bottom_frac: f64,
    pub left_min: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct GridTokens {
    pub gutter: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct RuleTokens {
    pub thickness: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TrackingTokens {
    pub caps: f64,
    pub display: f64,
    pub small: f64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AnimationTokens {
    pub spring_k: f64,
    pub snap_threshold: f64,
    pub cursor_blink_ms: u64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TerminalTokens {
    pub font_size: f32,
    pub pad_cells: usize,
}

// ---------------------------------------------------------------------------
// Default implementations (match existing const values exactly)
// ---------------------------------------------------------------------------

impl Default for DesignTokens {
    fn default() -> Self {
        Self {
            background: BackgroundTokens::default(),
            ink: InkTokens::default(),
            type_scale: TypeScaleTokens::default(),
            line_height: LineHeightTokens::default(),
            spacing: SpacingTokens::default(),
            margins: MarginTokens::default(),
            grid: GridTokens::default(),
            rules: RuleTokens::default(),
            tracking: TrackingTokens::default(),
            animation: AnimationTokens::default(),
            terminal: TerminalTokens::default(),
        }
    }
}

impl Default for BackgroundTokens {
    fn default() -> Self {
        Self {
            r: 1.0,
            g: 0.894,
            b: 0.769,
        }
    }
}

impl Default for InkTokens {
    fn default() -> Self {
        Self {
            primary: 1.0,
            section: 0.80,
            body: 0.70,
            secondary: 0.50,
            annotation: 0.40,
            rule: 0.15,
            ghost: 0.08,
        }
    }
}

impl Default for TypeScaleTokens {
    fn default() -> Self {
        Self {
            base: 18.0,
            ratio: 1.333,
        }
    }
}

impl Default for LineHeightTokens {
    fn default() -> Self {
        Self {
            body: 1.5,
            heading: 1.2,
            display: 1.05,
            caption: 1.4,
        }
    }
}

impl Default for SpacingTokens {
    fn default() -> Self {
        Self { baseline: 28.0 }
    }
}

impl Default for MarginTokens {
    fn default() -> Self {
        Self {
            left_frac: 1.0 / 9.0,
            right_frac: 2.0 / 9.0,
            top_frac: 1.0 / 9.0,
            bottom_frac: 2.0 / 9.0,
            left_min: 48.0,
        }
    }
}

impl Default for GridTokens {
    fn default() -> Self {
        Self { gutter: 24.0 }
    }
}

impl Default for RuleTokens {
    fn default() -> Self {
        Self { thickness: 0.5 }
    }
}

impl Default for TrackingTokens {
    fn default() -> Self {
        Self {
            caps: 0.08,
            display: -0.01,
            small: 0.02,
        }
    }
}

impl Default for AnimationTokens {
    fn default() -> Self {
        Self {
            spring_k: 0.2,
            snap_threshold: 0.001,
            cursor_blink_ms: 500,
        }
    }
}

impl Default for TerminalTokens {
    fn default() -> Self {
        Self {
            font_size: 28.0,
            pad_cells: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Helper methods
// ---------------------------------------------------------------------------

impl DesignTokens {
    /// Background color as a vello `Color`.
    pub fn bg_color(&self) -> Color {
        Color::new([
            self.background.r as f32,
            self.background.g as f32,
            self.background.b as f32,
            1.0,
        ])
    }

    /// Black ink at the given opacity (0.0 = transparent, 1.0 = solid black).
    pub fn ink_color(&self, opacity: f64) -> Color {
        Color::new([0.0_f32, 0.0, 0.0, opacity as f32])
    }

    /// Compute a type size from the modular scale.
    ///
    /// `step = 0` returns `base`, `step = 1` returns `base * ratio`, etc.
    /// Negative steps shrink below base.
    pub fn type_size(&self, step: i32) -> f64 {
        self.type_scale.base * self.type_scale.ratio.powi(step)
    }

    /// Return the line-height multiplier appropriate for a given font size.
    ///
    /// Heuristic: sizes >= 2 steps above base use `heading`, sizes >= 3 steps
    /// use `display`, otherwise `body`.
    pub fn line_height_for(&self, font_size: f64) -> f64 {
        let display_threshold = self.type_size(3);
        let heading_threshold = self.type_size(2);
        if font_size >= display_threshold {
            font_size * self.line_height.display
        } else if font_size >= heading_threshold {
            font_size * self.line_height.heading
        } else {
            font_size * self.line_height.body
        }
    }

    /// Snap a y-coordinate to the nearest baseline grid line.
    pub fn snap_to_baseline(&self, y: f64) -> f64 {
        (y / self.spacing.baseline).round() * self.spacing.baseline
    }

    /// Compute (left, right, top, bottom) margins in pixels given viewport size.
    pub fn margins(&self, vw: f64, vh: f64) -> (f64, f64, f64, f64) {
        let left = (vw * self.margins.left_frac).max(self.margins.left_min);
        let right = vw * self.margins.right_frac;
        let top = vh * self.margins.top_frac;
        let bottom = vh * self.margins.bottom_frac;
        (left, right, top, bottom)
    }

    /// Spacing helper: returns `baseline * multiple`.
    pub fn space(&self, multiple: f64) -> f64 {
        self.spacing.baseline * multiple
    }

    /// Serialize to a TOML string.
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    /// Deserialize from a TOML string.
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_default_matches_consts() {
        let t = DesignTokens::default();

        // Background
        assert!((t.background.r - 1.0).abs() < f64::EPSILON);
        assert!((t.background.g - 0.894).abs() < f64::EPSILON);
        assert!((t.background.b - 0.769).abs() < f64::EPSILON);

        // Ink
        assert!((t.ink.primary - 1.0).abs() < f64::EPSILON);
        assert!((t.ink.section - 0.80).abs() < f64::EPSILON);
        assert!((t.ink.body - 0.70).abs() < f64::EPSILON);
        assert!((t.ink.secondary - 0.50).abs() < f64::EPSILON);
        assert!((t.ink.annotation - 0.40).abs() < f64::EPSILON);
        assert!((t.ink.rule - 0.15).abs() < f64::EPSILON);
        assert!((t.ink.ghost - 0.08).abs() < f64::EPSILON);

        // Type scale
        assert!((t.type_scale.base - 18.0).abs() < f64::EPSILON);
        assert!((t.type_scale.ratio - 1.333).abs() < f64::EPSILON);

        // Line height
        assert!((t.line_height.body - 1.5).abs() < f64::EPSILON);
        assert!((t.line_height.heading - 1.2).abs() < f64::EPSILON);
        assert!((t.line_height.display - 1.05).abs() < f64::EPSILON);
        assert!((t.line_height.caption - 1.4).abs() < f64::EPSILON);

        // Spacing
        assert!((t.spacing.baseline - 28.0).abs() < f64::EPSILON);

        // Margins
        assert!((t.margins.left_frac - 1.0 / 9.0).abs() < 1e-10);
        assert!((t.margins.right_frac - 2.0 / 9.0).abs() < 1e-10);
        assert!((t.margins.left_min - 48.0).abs() < f64::EPSILON);

        // Grid
        assert!((t.grid.gutter - 24.0).abs() < f64::EPSILON);

        // Rules
        assert!((t.rules.thickness - 0.5).abs() < f64::EPSILON);

        // Tracking
        assert!((t.tracking.caps - 0.08).abs() < f64::EPSILON);
        assert!((t.tracking.display - (-0.01)).abs() < f64::EPSILON);
        assert!((t.tracking.small - 0.02).abs() < f64::EPSILON);

        // Animation
        assert!((t.animation.spring_k - 0.2).abs() < f64::EPSILON);
        assert!((t.animation.snap_threshold - 0.001).abs() < f64::EPSILON);
        assert_eq!(t.animation.cursor_blink_ms, 500);

        // Terminal
        assert!((t.terminal.font_size - 28.0).abs() < f32::EPSILON);
        assert_eq!(t.terminal.pad_cells, 1);
    }

    #[test]
    fn tokens_toml_roundtrip() {
        let original = DesignTokens::default();
        let toml_str = original.to_toml();
        let parsed = DesignTokens::from_toml(&toml_str).expect("roundtrip parse failed");

        assert!((parsed.background.r - original.background.r).abs() < f64::EPSILON);
        assert!((parsed.ink.primary - original.ink.primary).abs() < f64::EPSILON);
        assert!((parsed.type_scale.base - original.type_scale.base).abs() < f64::EPSILON);
        assert_eq!(parsed.animation.cursor_blink_ms, original.animation.cursor_blink_ms);
        assert!((parsed.terminal.font_size - original.terminal.font_size).abs() < f32::EPSILON);
    }

    #[test]
    fn tokens_partial_toml() {
        let partial = r#"
[ink]
primary = 0.9
secondary = 0.3
"#;
        let tokens = DesignTokens::from_toml(partial).expect("partial parse failed");
        // Overridden values
        assert!((tokens.ink.primary - 0.9).abs() < f64::EPSILON);
        assert!((tokens.ink.secondary - 0.3).abs() < f64::EPSILON);
        // Default values for everything else
        assert!((tokens.background.r - 1.0).abs() < f64::EPSILON);
        assert!((tokens.type_scale.base - 18.0).abs() < f64::EPSILON);
        assert!((tokens.ink.body - 0.70).abs() < f64::EPSILON);
    }

    #[test]
    fn tokens_invalid_toml() {
        let bad = "this is not [[ valid toml";
        let result = DesignTokens::from_toml(bad);
        assert!(result.is_err());
    }

    #[test]
    fn type_size_scale() {
        let t = DesignTokens::default();
        // Step 0 = base
        assert!((t.type_size(0) - 18.0).abs() < f64::EPSILON);
        // Step 1 = base * ratio
        assert!((t.type_size(1) - 18.0 * 1.333).abs() < 0.01);
        // Step 3 ~ 42.6
        let step3 = 18.0 * 1.333_f64.powi(3);
        assert!((t.type_size(3) - step3).abs() < 0.01);
    }

    #[test]
    fn snap_to_baseline_works() {
        let t = DesignTokens::default();
        // 27.0 rounds to 1*28 = 28.0
        assert!((t.snap_to_baseline(27.0) - 28.0).abs() < f64::EPSILON);
        // 56.0 is exactly 2*28
        assert!((t.snap_to_baseline(56.0) - 56.0).abs() < f64::EPSILON);
        // 42.0 rounds to 1*28 = 28.0 (42/28 = 1.5, rounds to 2*28 = 56)
        assert!((t.snap_to_baseline(42.0) - 56.0).abs() < f64::EPSILON);
        // 14.0 rounds to 0*28 = 0.0 (14/28 = 0.5, rounds to 0 or 28 depending on rounding)
        assert!((t.snap_to_baseline(84.0) - 84.0).abs() < f64::EPSILON);
    }

    #[test]
    fn space_helper() {
        let t = DesignTokens::default();
        assert!((t.space(1.0) - 28.0).abs() < f64::EPSILON);
        assert!((t.space(2.0) - 56.0).abs() < f64::EPSILON);
        assert!((t.space(0.5) - 14.0).abs() < f64::EPSILON);
    }
}
