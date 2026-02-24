//! Design REPL — interactive token editor rendered as a vello overlay.
//!
//! Toggle with Cmd+Shift+I. When active, keyboard input routes here instead
//! of the PTY / dashboard. Commands: set, get, list, save, reset, defaults,
//! help, exit.

use std::sync::{Arc, RwLock};

use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill, FontData};
use vello::{Glyph, Scene};
use winit::keyboard::{Key, NamedKey};

use crate::design::DesignTokens;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Output line styling.
#[derive(Debug, Clone)]
pub enum ReplOutputKind {
    Info,
    Success,
    Error,
    Value,
}

/// Self-contained Design REPL state.
pub struct DesignRepl {
    active: bool,
    input: String,
    history: Vec<String>,
    history_index: Option<usize>,
    output: Vec<(String, ReplOutputKind)>,
    scroll_offset: usize,
    cursor_visible: bool,
    last_blink: std::time::Instant,
    mono_font: Option<FontData>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

impl DesignRepl {
    pub fn new() -> Self {
        let mono_font = crate::dashboard::load_mono_font();
        Self {
            active: false,
            input: String::new(),
            history: Vec::new(),
            history_index: None,
            output: vec![
                ("Design REPL — type `help` for commands".into(), ReplOutputKind::Info),
            ],
            scroll_offset: 0,
            cursor_visible: true,
            last_blink: std::time::Instant::now(),
            mono_font,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    /// Handle a key press while the REPL is active.
    pub fn handle_key(&mut self, key: &Key, ctrl: bool, tokens: &Arc<RwLock<DesignTokens>>) {
        match key {
            Key::Named(NamedKey::Enter) => {
                let line = self.input.trim().to_string();
                if !line.is_empty() {
                    self.output.push((format!("> {}", line), ReplOutputKind::Info));
                    self.history.push(line.clone());
                    self.history_index = None;
                    self.execute(&line, tokens);
                    self.input.clear();
                    // Auto-scroll to bottom.
                    self.scroll_to_bottom();
                }
            }
            Key::Named(NamedKey::Backspace) => {
                if ctrl {
                    // Ctrl+Backspace: delete word
                    let trimmed = self.input.trim_end();
                    if let Some(pos) = trimmed.rfind(' ') {
                        self.input.truncate(pos + 1);
                    } else {
                        self.input.clear();
                    }
                } else {
                    self.input.pop();
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if !self.history.is_empty() {
                    let idx = match self.history_index {
                        Some(i) if i > 0 => i - 1,
                        Some(i) => i,
                        None => self.history.len() - 1,
                    };
                    self.history_index = Some(idx);
                    self.input = self.history[idx].clone();
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                if let Some(idx) = self.history_index {
                    if idx + 1 < self.history.len() {
                        self.history_index = Some(idx + 1);
                        self.input = self.history[idx + 1].clone();
                    } else {
                        self.history_index = None;
                        self.input.clear();
                    }
                }
            }
            Key::Named(NamedKey::Escape) => {
                self.active = false;
            }
            Key::Character(c) => {
                if ctrl && (c.as_str() == "c" || c.as_str() == "C") {
                    self.input.clear();
                } else if ctrl && (c.as_str() == "l" || c.as_str() == "L") {
                    self.output.clear();
                    self.scroll_offset = 0;
                } else {
                    self.input.push_str(c.as_str());
                }
            }
            _ => {}
        }
    }

    /// Render the REPL overlay onto `scene`.
    pub fn render(&mut self, scene: &mut Scene, width: f64, height: f64, tokens: &DesignTokens) {
        // Blink cursor.
        let now = std::time::Instant::now();
        if now.duration_since(self.last_blink).as_millis() >= tokens.animation.cursor_blink_ms as u128
        {
            self.cursor_visible = !self.cursor_visible;
            self.last_blink = now;
        }

        let pad = 32.0;
        let font_size: f32 = 20.0;
        let line_h = (font_size * 1.6) as f64;

        // Semi-transparent overlay.
        let overlay = Rect::new(0.0, 0.0, width, height);
        let overlay_color = Color::new([
            tokens.background.r as f32,
            tokens.background.g as f32,
            tokens.background.b as f32,
            0.97,
        ]);
        scene.fill(Fill::NonZero, Affine::IDENTITY, overlay_color, None, &overlay);

        // Title bar.
        let title_h = 48.0;
        let title_rect = Rect::new(0.0, 0.0, width, title_h);
        let title_bg = Color::new([0.0, 0.0, 0.0, (tokens.ink.ghost * 2.0).min(1.0) as f32]);
        scene.fill(Fill::NonZero, Affine::IDENTITY, title_bg, None, &title_rect);

        let title_color = tokens.ink_color(tokens.ink.primary);
        self.draw_text(scene, pad, 32.0, "DESIGN REPL", title_color, 24.0);

        let esc_hint = "Esc to close";
        self.draw_text(scene, width - pad - 150.0, 32.0, esc_hint, tokens.ink_color(tokens.ink.annotation), 16.0);

        // Output area.
        let output_top = title_h + pad;
        let input_h = line_h + pad;
        let output_bottom = height - input_h - pad;
        let visible_lines = ((output_bottom - output_top) / line_h).floor() as usize;

        // Clamp scroll.
        let max_scroll = self.output.len().saturating_sub(visible_lines);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }

        let start = self.scroll_offset;
        let end = (start + visible_lines).min(self.output.len());

        for (i, idx) in (start..end).enumerate() {
            let (ref text, ref kind) = self.output[idx];
            let y = output_top + (i as f64 + 1.0) * line_h;
            let color = match kind {
                ReplOutputKind::Info => tokens.ink_color(tokens.ink.body),
                ReplOutputKind::Success => Color::new([0.20, 0.65, 0.32, 1.0]),
                ReplOutputKind::Error => Color::new([0.80, 0.20, 0.18, 1.0]),
                ReplOutputKind::Value => tokens.ink_color(tokens.ink.primary),
            };
            self.draw_text(scene, pad, y, text, color, font_size as f64);
        }

        // Scroll indicator.
        if self.output.len() > visible_lines {
            let frac = self.scroll_offset as f64 / max_scroll.max(1) as f64;
            let track_h = output_bottom - output_top;
            let thumb_h = (track_h * visible_lines as f64 / self.output.len() as f64).max(20.0);
            let thumb_y = output_top + frac * (track_h - thumb_h);
            let track_x = width - 10.0;
            let thumb = Rect::new(track_x, thumb_y, track_x + 4.0, thumb_y + thumb_h);
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                tokens.ink_color(tokens.ink.rule),
                None,
                &thumb,
            );
        }

        // Input line.
        let input_y = height - pad;
        let prompt = "> ";
        let prompt_color = tokens.ink_color(tokens.ink.secondary);
        self.draw_text(scene, pad, input_y, prompt, prompt_color, font_size as f64);

        let prompt_w = self.measure_text(prompt, font_size);
        let input_color = tokens.ink_color(tokens.ink.primary);
        self.draw_text(
            scene,
            pad + prompt_w,
            input_y,
            &self.input,
            input_color,
            font_size as f64,
        );

        // Blinking cursor.
        if self.cursor_visible {
            let cursor_x = pad + prompt_w + self.measure_text(&self.input, font_size);
            let cursor_rect = Rect::new(
                cursor_x,
                input_y - font_size as f64 + 4.0,
                cursor_x + 2.0,
                input_y + 4.0,
            );
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                tokens.ink_color(tokens.ink.primary),
                None,
                &cursor_rect,
            );
        }

        // Separator above input.
        let sep_y = height - input_h - pad / 2.0;
        let sep = Rect::new(pad, sep_y, width - pad, sep_y + 1.0);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            tokens.ink_color(tokens.ink.rule),
            None,
            &sep,
        );
    }
}

// ---------------------------------------------------------------------------
// Command execution
// ---------------------------------------------------------------------------

impl DesignRepl {
    fn execute(&mut self, line: &str, tokens: &Arc<RwLock<DesignTokens>>) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return;
        }

        match parts[0] {
            "help" => self.cmd_help(),
            "exit" | "quit" => {
                self.active = false;
            }
            "get" => {
                if parts.len() < 2 {
                    self.push_error("Usage: get <path>");
                } else {
                    self.cmd_get(parts[1], tokens);
                }
            }
            "set" => {
                if parts.len() < 3 {
                    self.push_error("Usage: set <path> <value>");
                } else {
                    let value_str = parts[2..].join(" ");
                    self.cmd_set(parts[1], &value_str, tokens);
                }
            }
            "list" => {
                let section = parts.get(1).copied();
                self.cmd_list(section, tokens);
            }
            "save" => self.cmd_save(tokens),
            "reset" => self.cmd_reset(tokens),
            "defaults" => self.cmd_defaults(tokens),
            other => {
                self.push_error(&format!("Unknown command: `{}`. Type `help` for usage.", other));
            }
        }
    }

    fn cmd_help(&mut self) {
        let lines = [
            ("set <path> <value>", "Mutate a token (e.g. set bg.r 0.9)"),
            ("get <path>", "Show current value"),
            ("list [section]", "Show all tokens or a section"),
            ("save", "Write tokens to ~/.config/bisque-computer/design.toml"),
            ("reset", "Reload tokens from file"),
            ("defaults", "Reset to compiled defaults"),
            ("help", "Show this help"),
            ("exit", "Close the REPL"),
        ];
        self.output.push(("Commands:".into(), ReplOutputKind::Info));
        for (cmd, desc) in lines {
            self.output.push((
                format!("  {:<28} {}", cmd, desc),
                ReplOutputKind::Value,
            ));
        }
    }

    fn cmd_get(&mut self, path: &str, tokens: &Arc<RwLock<DesignTokens>>) {
        let t = tokens.read().unwrap();
        match resolve_get(&t, path) {
            Some(val) => self.output.push((format!("{} = {}", path, val), ReplOutputKind::Value)),
            None => self.push_error(&format!("Unknown path: `{}`", path)),
        }
    }

    fn cmd_set(&mut self, path: &str, value: &str, tokens: &Arc<RwLock<DesignTokens>>) {
        let mut t = tokens.write().unwrap();
        match resolve_set(&mut t, path, value) {
            Ok(msg) => self.output.push((msg, ReplOutputKind::Success)),
            Err(e) => self.push_error(&e),
        }
    }

    fn cmd_list(&mut self, section: Option<&str>, tokens: &Arc<RwLock<DesignTokens>>) {
        let t = tokens.read().unwrap();
        let entries = list_tokens(&t, section);
        if entries.is_empty() {
            if let Some(s) = section {
                self.push_error(&format!("Unknown section: `{}`", s));
            } else {
                self.push_error("No tokens found");
            }
        } else {
            for (path, val) in entries {
                self.output
                    .push((format!("  {:<32} {}", path, val), ReplOutputKind::Value));
            }
        }
    }

    fn cmd_save(&mut self, tokens: &Arc<RwLock<DesignTokens>>) {
        let t = tokens.read().unwrap();
        let toml_str = t.to_toml();
        drop(t);

        let mut path = crate::home_dir();
        path.push(".config");
        path.push("bisque-computer");
        if let Err(e) = std::fs::create_dir_all(&path) {
            self.push_error(&format!("Cannot create config dir: {}", e));
            return;
        }
        path.push("design.toml");

        match std::fs::write(&path, &toml_str) {
            Ok(()) => self.output.push((
                format!("Saved to {}", path.display()),
                ReplOutputKind::Success,
            )),
            Err(e) => self.push_error(&format!("Write failed: {}", e)),
        }
    }

    fn cmd_reset(&mut self, tokens: &Arc<RwLock<DesignTokens>>) {
        let mut path = crate::home_dir();
        path.push(".config");
        path.push("bisque-computer");
        path.push("design.toml");

        if !path.exists() {
            self.push_error("No design.toml found — nothing to reload");
            return;
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match DesignTokens::from_toml(&content) {
                Ok(new_tokens) => {
                    *tokens.write().unwrap() = new_tokens;
                    self.output.push((
                        format!("Reloaded from {}", path.display()),
                        ReplOutputKind::Success,
                    ));
                }
                Err(e) => self.push_error(&format!("Parse error: {}", e)),
            },
            Err(e) => self.push_error(&format!("Read failed: {}", e)),
        }
    }

    fn cmd_defaults(&mut self, tokens: &Arc<RwLock<DesignTokens>>) {
        *tokens.write().unwrap() = DesignTokens::default();
        self.output.push((
            "Reset to compiled defaults".into(),
            ReplOutputKind::Success,
        ));
    }

    fn push_error(&mut self, msg: &str) {
        self.output.push((msg.to_string(), ReplOutputKind::Error));
    }

    fn scroll_to_bottom(&mut self) {
        // Will be clamped in render.
        self.scroll_offset = self.output.len();
    }
}

// ---------------------------------------------------------------------------
// Text rendering helpers (mirrors dashboard.rs approach)
// ---------------------------------------------------------------------------

impl DesignRepl {
    fn draw_text(
        &self,
        scene: &mut Scene,
        x: f64,
        y: f64,
        text: &str,
        color: Color,
        size: f64,
    ) {
        if let Some(ref font) = self.mono_font {
            let font_size = size as f32;
            let glyphs = layout_glyphs(text, font, font_size, x, y);
            if !glyphs.is_empty() {
                scene
                    .draw_glyphs(font)
                    .font_size(font_size)
                    .brush(&color)
                    .draw(Fill::NonZero, glyphs.into_iter());
            }
        } else {
            // Bitmap fallback via dashboard.
            crate::dashboard::draw_text_pub(scene, x, y, text, color, size, None);
        }
    }

    fn measure_text(&self, text: &str, font_size: f32) -> f64 {
        if let Some(ref font) = self.mono_font {
            measure_text_width(text, font, font_size)
        } else {
            // Rough bitmap estimate.
            let scale = font_size as f64 / 14.0;
            text.len() as f64 * (7.0 * scale + 1.0 * scale)
        }
    }
}

// ---------------------------------------------------------------------------
// Glyph layout (same approach as dashboard.rs layout_text_single_line)
// ---------------------------------------------------------------------------

fn layout_glyphs(text: &str, font_data: &FontData, font_size: f32, start_x: f64, start_y: f64) -> Vec<Glyph> {
    let font_ref = match skrifa::FontRef::from_index(font_data.data.as_ref(), font_data.index) {
        Ok(f) => f,
        Err(_) => return vec![],
    };

    use skrifa::MetadataProvider;
    let charmap = font_ref.charmap();
    let glyph_metrics = font_ref.glyph_metrics(
        skrifa::instance::Size::new(font_size),
        skrifa::instance::LocationRef::default(),
    );

    let mut glyphs = Vec::new();
    let mut x = start_x;

    for ch in text.chars() {
        let gid = charmap.map(ch).unwrap_or_default();
        let advance = glyph_metrics
            .advance_width(gid)
            .unwrap_or(font_size * 0.5) as f64;

        if ch != ' ' {
            glyphs.push(Glyph {
                id: gid.to_u32(),
                x: x as f32,
                y: start_y as f32,
            });
        }

        x += advance;
    }

    glyphs
}

fn measure_text_width(text: &str, font_data: &FontData, font_size: f32) -> f64 {
    let font_ref = match skrifa::FontRef::from_index(font_data.data.as_ref(), font_data.index) {
        Ok(f) => f,
        Err(_) => return text.len() as f64 * font_size as f64 * 0.6,
    };

    use skrifa::MetadataProvider;
    let charmap = font_ref.charmap();
    let glyph_metrics = font_ref.glyph_metrics(
        skrifa::instance::Size::new(font_size),
        skrifa::instance::LocationRef::default(),
    );

    let mut width = 0.0_f64;
    for ch in text.chars() {
        let gid = charmap.map(ch).unwrap_or_default();
        let advance = glyph_metrics
            .advance_width(gid)
            .unwrap_or(font_size * 0.5) as f64;
        width += advance;
    }
    width
}

// ---------------------------------------------------------------------------
// Token path resolution
// ---------------------------------------------------------------------------

fn resolve_get(t: &DesignTokens, path: &str) -> Option<String> {
    Some(match path {
        // background
        "bg" => format!("rgb({:.3}, {:.3}, {:.3})", t.background.r, t.background.g, t.background.b),
        "bg.r" => format!("{:.3}", t.background.r),
        "bg.g" => format!("{:.3}", t.background.g),
        "bg.b" => format!("{:.3}", t.background.b),

        // ink
        "ink.primary" => format!("{:.3}", t.ink.primary),
        "ink.section" => format!("{:.3}", t.ink.section),
        "ink.body" => format!("{:.3}", t.ink.body),
        "ink.secondary" => format!("{:.3}", t.ink.secondary),
        "ink.annotation" => format!("{:.3}", t.ink.annotation),
        "ink.rule" => format!("{:.3}", t.ink.rule),
        "ink.ghost" => format!("{:.3}", t.ink.ghost),

        // type scale
        "type.base" => format!("{:.1}", t.type_scale.base),
        "type.ratio" => format!("{:.3}", t.type_scale.ratio),

        // line height
        "line_height.body" => format!("{:.2}", t.line_height.body),
        "line_height.heading" => format!("{:.2}", t.line_height.heading),
        "line_height.display" => format!("{:.2}", t.line_height.display),
        "line_height.caption" => format!("{:.2}", t.line_height.caption),

        // spacing
        "spacing.baseline" => format!("{:.1}", t.spacing.baseline),

        // margins
        "margins.left_frac" => format!("{:.4}", t.margins.left_frac),
        "margins.right_frac" => format!("{:.4}", t.margins.right_frac),
        "margins.top_frac" => format!("{:.4}", t.margins.top_frac),
        "margins.bottom_frac" => format!("{:.4}", t.margins.bottom_frac),
        "margins.left_min" => format!("{:.1}", t.margins.left_min),

        // grid
        "grid.gutter" => format!("{:.1}", t.grid.gutter),

        // rules
        "rules.thickness" => format!("{:.2}", t.rules.thickness),

        // tracking
        "tracking.caps" => format!("{:.3}", t.tracking.caps),
        "tracking.display" => format!("{:.3}", t.tracking.display),
        "tracking.small" => format!("{:.3}", t.tracking.small),

        // animation
        "animation.spring_k" => format!("{:.3}", t.animation.spring_k),
        "animation.snap_threshold" => format!("{:.4}", t.animation.snap_threshold),
        "animation.cursor_blink_ms" => format!("{}", t.animation.cursor_blink_ms),

        // terminal
        "terminal.font_size" => format!("{:.1}", t.terminal.font_size),
        "terminal.pad_cells" => format!("{}", t.terminal.pad_cells),

        _ => return None,
    })
}

fn resolve_set(t: &mut DesignTokens, path: &str, value: &str) -> Result<String, String> {
    // Parse helper for f64
    let parse_f64 = |v: &str| -> Result<f64, String> {
        v.parse::<f64>().map_err(|_| format!("Invalid number: `{}`", v))
    };
    let parse_f32 = |v: &str| -> Result<f32, String> {
        v.parse::<f32>().map_err(|_| format!("Invalid number: `{}`", v))
    };
    let parse_u64 = |v: &str| -> Result<u64, String> {
        v.parse::<u64>().map_err(|_| format!("Invalid integer: `{}`", v))
    };
    let parse_usize = |v: &str| -> Result<usize, String> {
        v.parse::<usize>().map_err(|_| format!("Invalid integer: `{}`", v))
    };

    match path {
        // background — accept single value as RGB shorthand or dotted path
        "bg" => {
            // Try parsing as hex color #RRGGBB
            if let Some(stripped) = value.strip_prefix('#') {
                if stripped.len() == 6 {
                    let r = u8::from_str_radix(&stripped[0..2], 16).map_err(|_| "Invalid hex color".to_string())?;
                    let g = u8::from_str_radix(&stripped[2..4], 16).map_err(|_| "Invalid hex color".to_string())?;
                    let b = u8::from_str_radix(&stripped[4..6], 16).map_err(|_| "Invalid hex color".to_string())?;
                    t.background.r = r as f64 / 255.0;
                    t.background.g = g as f64 / 255.0;
                    t.background.b = b as f64 / 255.0;
                    return Ok(format!("bg = rgb({:.3}, {:.3}, {:.3})", t.background.r, t.background.g, t.background.b));
                }
            }
            return Err("Use `bg #RRGGBB` or `bg.r`, `bg.g`, `bg.b` individually".into());
        }
        "bg.r" => { t.background.r = parse_f64(value)?; }
        "bg.g" => { t.background.g = parse_f64(value)?; }
        "bg.b" => { t.background.b = parse_f64(value)?; }

        "ink.primary" => { t.ink.primary = parse_f64(value)?; }
        "ink.section" => { t.ink.section = parse_f64(value)?; }
        "ink.body" => { t.ink.body = parse_f64(value)?; }
        "ink.secondary" => { t.ink.secondary = parse_f64(value)?; }
        "ink.annotation" => { t.ink.annotation = parse_f64(value)?; }
        "ink.rule" => { t.ink.rule = parse_f64(value)?; }
        "ink.ghost" => { t.ink.ghost = parse_f64(value)?; }

        "type.base" => { t.type_scale.base = parse_f64(value)?; }
        "type.ratio" => { t.type_scale.ratio = parse_f64(value)?; }

        "line_height.body" => { t.line_height.body = parse_f64(value)?; }
        "line_height.heading" => { t.line_height.heading = parse_f64(value)?; }
        "line_height.display" => { t.line_height.display = parse_f64(value)?; }
        "line_height.caption" => { t.line_height.caption = parse_f64(value)?; }

        "spacing.baseline" => { t.spacing.baseline = parse_f64(value)?; }

        "margins.left_frac" => { t.margins.left_frac = parse_f64(value)?; }
        "margins.right_frac" => { t.margins.right_frac = parse_f64(value)?; }
        "margins.top_frac" => { t.margins.top_frac = parse_f64(value)?; }
        "margins.bottom_frac" => { t.margins.bottom_frac = parse_f64(value)?; }
        "margins.left_min" => { t.margins.left_min = parse_f64(value)?; }

        "grid.gutter" => { t.grid.gutter = parse_f64(value)?; }

        "rules.thickness" => { t.rules.thickness = parse_f64(value)?; }

        "tracking.caps" => { t.tracking.caps = parse_f64(value)?; }
        "tracking.display" => { t.tracking.display = parse_f64(value)?; }
        "tracking.small" => { t.tracking.small = parse_f64(value)?; }

        "animation.spring_k" => { t.animation.spring_k = parse_f64(value)?; }
        "animation.snap_threshold" => { t.animation.snap_threshold = parse_f64(value)?; }
        "animation.cursor_blink_ms" => { t.animation.cursor_blink_ms = parse_u64(value)?; }

        "terminal.font_size" => { t.terminal.font_size = parse_f32(value)?; }
        "terminal.pad_cells" => { t.terminal.pad_cells = parse_usize(value)?; }

        _ => return Err(format!("Unknown path: `{}`. Type `list` to see all paths.", path)),
    }

    // For paths that don't return early, report the new value.
    let display = resolve_get(t, path).unwrap_or_else(|| value.to_string());
    Ok(format!("{} = {}", path, display))
}

fn list_tokens(t: &DesignTokens, section: Option<&str>) -> Vec<(String, String)> {
    let mut out = Vec::new();

    let all = section.is_none();

    if all || section == Some("bg") || section == Some("background") {
        out.push(("bg.r".into(), format!("{:.3}", t.background.r)));
        out.push(("bg.g".into(), format!("{:.3}", t.background.g)));
        out.push(("bg.b".into(), format!("{:.3}", t.background.b)));
    }

    if all || section == Some("ink") {
        out.push(("ink.primary".into(), format!("{:.3}", t.ink.primary)));
        out.push(("ink.section".into(), format!("{:.3}", t.ink.section)));
        out.push(("ink.body".into(), format!("{:.3}", t.ink.body)));
        out.push(("ink.secondary".into(), format!("{:.3}", t.ink.secondary)));
        out.push(("ink.annotation".into(), format!("{:.3}", t.ink.annotation)));
        out.push(("ink.rule".into(), format!("{:.3}", t.ink.rule)));
        out.push(("ink.ghost".into(), format!("{:.3}", t.ink.ghost)));
    }

    if all || section == Some("type") || section == Some("type_scale") {
        out.push(("type.base".into(), format!("{:.1}", t.type_scale.base)));
        out.push(("type.ratio".into(), format!("{:.3}", t.type_scale.ratio)));
    }

    if all || section == Some("line_height") {
        out.push(("line_height.body".into(), format!("{:.2}", t.line_height.body)));
        out.push(("line_height.heading".into(), format!("{:.2}", t.line_height.heading)));
        out.push(("line_height.display".into(), format!("{:.2}", t.line_height.display)));
        out.push(("line_height.caption".into(), format!("{:.2}", t.line_height.caption)));
    }

    if all || section == Some("spacing") {
        out.push(("spacing.baseline".into(), format!("{:.1}", t.spacing.baseline)));
    }

    if all || section == Some("margins") {
        out.push(("margins.left_frac".into(), format!("{:.4}", t.margins.left_frac)));
        out.push(("margins.right_frac".into(), format!("{:.4}", t.margins.right_frac)));
        out.push(("margins.top_frac".into(), format!("{:.4}", t.margins.top_frac)));
        out.push(("margins.bottom_frac".into(), format!("{:.4}", t.margins.bottom_frac)));
        out.push(("margins.left_min".into(), format!("{:.1}", t.margins.left_min)));
    }

    if all || section == Some("grid") {
        out.push(("grid.gutter".into(), format!("{:.1}", t.grid.gutter)));
    }

    if all || section == Some("rules") {
        out.push(("rules.thickness".into(), format!("{:.2}", t.rules.thickness)));
    }

    if all || section == Some("tracking") {
        out.push(("tracking.caps".into(), format!("{:.3}", t.tracking.caps)));
        out.push(("tracking.display".into(), format!("{:.3}", t.tracking.display)));
        out.push(("tracking.small".into(), format!("{:.3}", t.tracking.small)));
    }

    if all || section == Some("animation") {
        out.push(("animation.spring_k".into(), format!("{:.3}", t.animation.spring_k)));
        out.push(("animation.snap_threshold".into(), format!("{:.4}", t.animation.snap_threshold)));
        out.push(("animation.cursor_blink_ms".into(), format!("{}", t.animation.cursor_blink_ms)));
    }

    if all || section == Some("terminal") {
        out.push(("terminal.font_size".into(), format!("{:.1}", t.terminal.font_size)));
        out.push(("terminal.pad_cells".into(), format!("{}", t.terminal.pad_cells)));
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repl_new_is_inactive() {
        let repl = DesignRepl::new();
        assert!(!repl.is_active());
    }

    #[test]
    fn repl_toggle() {
        let mut repl = DesignRepl::new();
        repl.toggle();
        assert!(repl.is_active());
        repl.toggle();
        assert!(!repl.is_active());
    }

    #[test]
    fn resolve_get_bg() {
        let t = DesignTokens::default();
        let val = resolve_get(&t, "bg.r").unwrap();
        assert_eq!(val, "1.000");
    }

    #[test]
    fn resolve_get_ink() {
        let t = DesignTokens::default();
        assert_eq!(resolve_get(&t, "ink.primary").unwrap(), "1.000");
        assert_eq!(resolve_get(&t, "ink.body").unwrap(), "0.700");
    }

    #[test]
    fn resolve_get_unknown() {
        let t = DesignTokens::default();
        assert!(resolve_get(&t, "nonexistent").is_none());
    }

    #[test]
    fn resolve_set_f64() {
        let mut t = DesignTokens::default();
        let result = resolve_set(&mut t, "ink.body", "0.65");
        assert!(result.is_ok());
        assert!((t.ink.body - 0.65).abs() < f64::EPSILON);
    }

    #[test]
    fn resolve_set_hex_bg() {
        let mut t = DesignTokens::default();
        let result = resolve_set(&mut t, "bg", "#FAEBD7");
        assert!(result.is_ok());
        assert!((t.background.r - 0xFA as f64 / 255.0).abs() < 0.01);
    }

    #[test]
    fn resolve_set_invalid_number() {
        let mut t = DesignTokens::default();
        let result = resolve_set(&mut t, "ink.body", "not_a_number");
        assert!(result.is_err());
    }

    #[test]
    fn resolve_set_unknown_path() {
        let mut t = DesignTokens::default();
        let result = resolve_set(&mut t, "nonexistent", "42");
        assert!(result.is_err());
    }

    #[test]
    fn list_all_tokens() {
        let t = DesignTokens::default();
        let entries = list_tokens(&t, None);
        assert!(entries.len() > 20);
    }

    #[test]
    fn list_section() {
        let t = DesignTokens::default();
        let entries = list_tokens(&t, Some("ink"));
        assert_eq!(entries.len(), 7);
    }

    #[test]
    fn list_unknown_section() {
        let t = DesignTokens::default();
        let entries = list_tokens(&t, Some("nonexistent"));
        assert!(entries.is_empty());
    }

    #[test]
    fn execute_help() {
        let mut repl = DesignRepl::new();
        let tokens = Arc::new(RwLock::new(DesignTokens::default()));
        repl.execute("help", &tokens);
        assert!(repl.output.len() > 2);
    }

    #[test]
    fn execute_get_set() {
        let mut repl = DesignRepl::new();
        let tokens = Arc::new(RwLock::new(DesignTokens::default()));
        repl.execute("set ink.body 0.55", &tokens);
        let t = tokens.read().unwrap();
        assert!((t.ink.body - 0.55).abs() < f64::EPSILON);
        drop(t);
        repl.execute("get ink.body", &tokens);
        let last = &repl.output.last().unwrap().0;
        assert!(last.contains("0.550"));
    }

    #[test]
    fn execute_defaults() {
        let mut repl = DesignRepl::new();
        let tokens = Arc::new(RwLock::new(DesignTokens::default()));
        // Modify a value.
        tokens.write().unwrap().ink.body = 0.1;
        repl.execute("defaults", &tokens);
        let t = tokens.read().unwrap();
        assert!((t.ink.body - 0.70).abs() < f64::EPSILON);
    }
}
