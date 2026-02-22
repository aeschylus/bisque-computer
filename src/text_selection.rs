//! Interactive text selection powered by `parley` `PlainEditor`.
//!
//! This module provides a `SelectableText` widget that wraps a `parley`
//! `PlainEditor` and integrates with the vello scene graph for rendering.
//!
//! Design:
//! - `SelectableText` owns one `PlainEditor<ColorBrush>` per region.
//! - Mouse events (click, drag) are forwarded via `handle_mouse_press` and
//!   `handle_mouse_drag`.
//! - Cmd+C / Ctrl+C copies `editor.selected_text()` to the clipboard.
//! - `render_into_scene` draws the text, selection highlight rects, and cursor.
//!
//! The `FontContext` and `LayoutContext` are shared across all `SelectableText`
//! instances held by the app; they live on the `App` struct.
//!
//! ## Brush type
//!
//! `parley`'s `Brush` trait requires `Clone + PartialEq + Default + Debug`.
//! `vello::peniko::Color` (which is `AlphaColor<Srgb>`) does not implement
//! `Default`, so we wrap it in a `ColorBrush` newtype that provides `Default`
//! as opaque black.

use parley::{
    FontContext, LayoutContext,
    editing::PlainEditor,
    layout::{Alignment, PositionedLayoutItem},
};
use vello::{
    Glyph, Scene,
    kurbo::{Affine, Rect},
    peniko::{Color, Fill},
};

// ---------------------------------------------------------------------------
// ColorBrush — newtype satisfying parley's Brush trait
// ---------------------------------------------------------------------------

/// A thin wrapper around `vello::peniko::Color` that satisfies `parley::Brush`.
///
/// `parley::Brush` requires `Clone + PartialEq + Default + Debug`. `peniko::Color`
/// (alias for `AlphaColor<Srgb>`) does not implement `Default`, so this newtype
/// provides `Default` as opaque black.
///
/// `parley` has a blanket impl `impl<T: Clone + PartialEq + Default + Debug> Brush for T`,
/// so we do not need to write an explicit `impl Brush for ColorBrush {}`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorBrush(pub Color);

impl Default for ColorBrush {
    fn default() -> Self {
        // Opaque black — always visible against the bisque background.
        ColorBrush(Color::new([0.0_f32, 0.0, 0.0, 1.0]))
    }
}

// ---------------------------------------------------------------------------
// Shared parley contexts
// ---------------------------------------------------------------------------

/// Shared font context used by all `SelectableText` instances.
///
/// Parley discovers system fonts lazily; this struct holds the cache.
pub struct ParleyCtx {
    pub font_cx: FontContext,
    pub layout_cx: LayoutContext<ColorBrush>,
}

impl ParleyCtx {
    pub fn new() -> Self {
        Self {
            font_cx: FontContext::new(),
            layout_cx: LayoutContext::new(),
        }
    }
}

impl Default for ParleyCtx {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SelectableText
// ---------------------------------------------------------------------------

/// A single region of selectable, read-only text rendered via parley.
pub struct SelectableText {
    editor: PlainEditor<ColorBrush>,
    /// Top-left origin of this text region in window coordinates.
    origin: (f64, f64),
    /// Width budget for line-wrapping. `None` means unbounded (single line).
    width: Option<f32>,
    /// Whether the mouse button is currently held (drag-select in progress).
    is_dragging: bool,
}

impl SelectableText {
    /// Create a new selectable text region at the given position.
    ///
    /// `font_size` is in logical pixels (will be passed to parley as-is).
    /// `text` is the content to display (read-only; set once).
    pub fn new(
        text: &str,
        font_size: f32,
        origin: (f64, f64),
        width: Option<f32>,
        ctx: &mut ParleyCtx,
    ) -> Self {
        let mut editor = PlainEditor::new(font_size);
        editor.set_width(width);
        editor.set_alignment(Alignment::Start);
        // Push text into the editor's buffer.
        {
            let mut driver = editor.driver(&mut ctx.font_cx, &mut ctx.layout_cx);
            driver.insert_or_replace_selection(text);
            // Move cursor away from selection so nothing is pre-selected.
            driver.move_to_text_start();
        }
        Self {
            editor,
            origin,
            width,
            is_dragging: false,
        }
    }

    /// Update the displayed text (triggers a re-layout on next render).
    pub fn set_text(&mut self, text: &str, ctx: &mut ParleyCtx) {
        let mut driver = self.editor.driver(&mut ctx.font_cx, &mut ctx.layout_cx);
        driver.select_all();
        driver.insert_or_replace_selection(text);
        driver.move_to_text_start();
    }

    /// Update the origin (repositions without re-laying out text).
    pub fn set_origin(&mut self, origin: (f64, f64)) {
        self.origin = origin;
    }

    /// Update the line-wrap width and re-layout.
    pub fn set_width(&mut self, width: Option<f32>) {
        self.width = width;
        self.editor.set_width(width);
    }

    // ------------------------------------------------------------------
    // Event handling
    // ------------------------------------------------------------------

    /// Handle a mouse press event (single click moves cursor; initiates drag).
    ///
    /// `window_x`, `window_y` are the cursor position in window coordinates.
    pub fn handle_mouse_press(
        &mut self,
        window_x: f64,
        window_y: f64,
        ctx: &mut ParleyCtx,
    ) {
        let local = self.to_local(window_x, window_y);
        let mut driver = self.editor.driver(&mut ctx.font_cx, &mut ctx.layout_cx);
        driver.move_to_point(local.0, local.1);
        self.is_dragging = true;
    }

    /// Handle cursor movement during a drag (extends selection).
    pub fn handle_mouse_drag(
        &mut self,
        window_x: f64,
        window_y: f64,
        ctx: &mut ParleyCtx,
    ) {
        if !self.is_dragging {
            return;
        }
        let local = self.to_local(window_x, window_y);
        let mut driver = self.editor.driver(&mut ctx.font_cx, &mut ctx.layout_cx);
        driver.extend_selection_to_point(local.0, local.1);
    }

    /// Handle mouse button release (ends drag).
    pub fn handle_mouse_release(&mut self) {
        self.is_dragging = false;
    }

    /// Return the currently selected text, if any.
    pub fn selected_text(&self) -> Option<&str> {
        self.editor.selected_text()
    }

    // ------------------------------------------------------------------
    // Hit testing
    // ------------------------------------------------------------------

    /// Test whether a window-coordinate point falls within this text region.
    pub fn hit_test(&self, window_x: f64, window_y: f64) -> bool {
        if let Some(layout) = self.editor.try_layout() {
            let local = self.to_local(window_x, window_y);
            let w = self.width.map(|w| w as f32).unwrap_or(layout.width());
            let h = layout.height();
            local.0 >= 0.0 && local.0 <= w && local.1 >= 0.0 && local.1 <= h
        } else {
            false
        }
    }

    // ------------------------------------------------------------------
    // Rendering
    // ------------------------------------------------------------------

    /// Render text, selection highlights, and cursor into the vello scene.
    ///
    /// `text_color` is the foreground color for unselected text.
    /// `selection_color` is the fill color for selection highlight rects.
    pub fn render_into_scene(
        &mut self,
        scene: &mut Scene,
        text_color: Color,
        selection_color: Color,
        ctx: &mut ParleyCtx,
    ) {
        let (ox, oy) = self.origin;
        let transform = Affine::translate((ox, oy));

        // --- Selection highlight rects ---
        // Collect highlight rects before borrowing the layout so the two
        // `self.editor` borrows do not overlap.
        // parley::BoundingBox uses x0, y0, x1, y1 fields.
        let mut highlight_rects: Vec<Rect> = Vec::new();
        self.editor.selection_geometry_with(|bbox, _style_idx| {
            highlight_rects.push(Rect::new(
                bbox.x0 as f64,
                bbox.y0 as f64,
                bbox.x1 as f64,
                bbox.y1 as f64,
            ));
        });
        for rect in &highlight_rects {
            scene.fill(Fill::NonZero, transform, selection_color, None, rect);
        }

        // --- Glyph runs ---
        let layout = self.editor.layout(&mut ctx.font_cx, &mut ctx.layout_cx);
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };

                let font_data = glyph_run.run().font();
                let font_size = glyph_run.run().font_size();
                let style = glyph_run.style();

                // Use the style brush color, falling back to text_color.
                // style.brush is `ColorBrush` (not Option); `.0` extracts the Color.
                let brush_color = style.brush.0;
                // If brush is default (opaque black from Default::default()), use
                // the caller-supplied text_color so the caller can override it.
                let effective_color = if brush_color == ColorBrush::default().0 {
                    text_color
                } else {
                    brush_color
                };

                let vello_glyphs: Vec<Glyph> = glyph_run
                    .positioned_glyphs()
                    .map(|g| Glyph { id: g.id, x: g.x, y: g.y })
                    .collect();

                if vello_glyphs.is_empty() {
                    continue;
                }

                scene
                    .draw_glyphs(font_data)
                    .font_size(font_size)
                    .transform(transform)
                    .brush(&effective_color)
                    .draw(Fill::NonZero, vello_glyphs.into_iter());
            }
        }
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Convert window coordinates to layout-local coordinates.
    fn to_local(&self, window_x: f64, window_y: f64) -> (f32, f32) {
        ((window_x - self.origin.0) as f32, (window_y - self.origin.1) as f32)
    }
}
