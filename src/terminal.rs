//! Backend-agnostic terminal emulator for the bisque-computer terminal screen.
//!
//! Provides `TerminalPane` — a struct that:
//!   - Connects to a byte-stream backend (local PTY or VM serial console)
//!   - Feeds output to `alacritty_terminal::Term` via a tokio mpsc channel
//!   - Exposes `render_into_scene()` to draw the cell grid using vello
//!   - Routes keyboard input from winit to the backend writer
//!
//! The Cascadia Code font is embedded in the binary via `include_bytes!()`.
//!
//! Architecture (data flows):
//!
//! ```text
//! [backend: local PTY / VM serial console]
//!       │ byte stream (read)
//!       ▼
//! [reader thread/task] ──mpsc──► [main thread: drain_output()]
//!                                          │
//!                                          ▼
//!                              alacritty_terminal::Term
//!                                          │
//!                                          ▼
//!                              render_into_scene() → vello Scene
//! ```
//!
//! Key input:
//! ```text
//! winit KeyEvent ──► key_event_to_pty_bytes() ──► backend writer (sync write)
//! ```

use std::io::Write;
use std::sync::{Arc, Mutex};

use alacritty_terminal::Term;
use alacritty_terminal::event::EventListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::Config;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color as AlaColor, NamedColor, Processor, Rgb};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use tokio::sync::mpsc as tokio_mpsc;
use vello::Glyph;
use vello::Scene;
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill, FontData};
use winit::keyboard::{Key, NamedKey};

// Cascadia Code embedded as fallback (SIL OFL license).
// Attribution: Copyright (c) Microsoft Corporation
const CASCADIA_CODE_BYTES: &[u8] = include_bytes!("../assets/CascadiaCode.ttf");

/// Terminal background: bisque beige (matches app style guide).
const TERM_BG: Color = Color::new([1.0, 0.894, 0.769, 1.0]);
/// Default foreground: black (matches app style guide).
const TERM_FG: Color = Color::new([0.0, 0.0, 0.0, 1.0]);
/// Cursor color: dark brown, visible on bisque.
const TERM_CURSOR: Color = Color::new([0.40, 0.26, 0.13, 0.85]);

/// Horizontal padding (one cell width on each side).
const TERM_PAD_CELLS: usize = 1;

/// Monaco font stack paths (code/mono font per style guide).
const MONO_FONT_PATHS: &[&str] = &[
    "/System/Library/Fonts/Monaco.ttf",
    "/System/Library/Fonts/Supplemental/Menlo.ttc",
    "/Library/Fonts/Monaco.ttf",
];

/// Load Monaco (or fallback mono font) from system, falling back to embedded Cascadia Code.
fn load_terminal_font() -> FontData {
    for path in MONO_FONT_PATHS {
        if let Ok(data) = std::fs::read(path) {
            return FontData::new(data.into(), 0);
        }
    }
    FontData::new(CASCADIA_CODE_BYTES.to_vec().into(), 0)
}

/// Default font size for the terminal in pixels.
const DEFAULT_FONT_SIZE: f32 = 28.0;
/// Minimum font size.
const MIN_FONT_SIZE: f32 = 8.0;
/// Maximum font size.
const MAX_FONT_SIZE: f32 = 72.0;
/// Font size step per Cmd+=/Cmd+-.
const FONT_SIZE_STEP: f32 = 2.0;

// --- TermSize helper --------------------------------------------------------

struct TermSize {
    columns: usize,
    screen_lines: usize,
}

impl TermSize {
    fn new(columns: usize, screen_lines: usize) -> Self {
        Self { columns, screen_lines }
    }
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }
    fn screen_lines(&self) -> usize {
        self.screen_lines
    }
    fn columns(&self) -> usize {
        self.columns
    }
}

// --- No-op EventListener ----------------------------------------------------

#[derive(Clone)]
struct TermEventListener;

impl EventListener for TermEventListener {}

// --- PtyResize trait ---------------------------------------------------------

/// Trait for resizing the remote terminal, abstracted over backends.
///
/// Implemented by `LocalPtyResize` (portable_pty) and `VmPtyResize` (Virtualization.framework).
pub trait PtyResize: Send {
    fn resize(&self, rows: u16, cols: u16, pixel_width: u16, pixel_height: u16);
}

/// Adapter for local PTY resize via `portable_pty::MasterPty`.
struct LocalPtyResize(Box<dyn portable_pty::MasterPty + Send>);

impl PtyResize for LocalPtyResize {
    fn resize(&self, rows: u16, cols: u16, pixel_width: u16, pixel_height: u16) {
        let _ = self.0.resize(PtySize {
            rows,
            cols,
            pixel_width,
            pixel_height,
        });
    }
}

// --- TerminalPane -----------------------------------------------------------

/// A terminal pane that renders into a vello `Scene`.
///
/// Backend-agnostic: can be driven by a local PTY (`spawn()`) or by
/// pre-wired byte streams from a VM serial console (`from_streams()`).
pub struct TerminalPane {
    /// The terminal state machine (cell grid + cursor + SGR state).
    /// Wrapped in Arc<Mutex<>> so the reader thread can send output to it.
    term: Arc<Mutex<Term<TermEventListener>>>,

    /// VTE ANSI parser (drives `Term` with bytes from the PTY).
    /// Owned by drain_output() on the main thread.
    processor: Processor,

    /// Buffered PTY output bytes from the reader thread.
    rx: tokio_mpsc::UnboundedReceiver<Vec<u8>>,

    /// Write handle to the backend (sends input to the shell/VM).
    pty_writer: Box<dyn Write + Send>,

    /// Resize handle (backend-agnostic).
    pty_resize: Box<dyn PtyResize + Send>,

    /// Cascadia Code font data.
    font: FontData,

    /// Current font size in pixels.
    pub font_size: f32,

    /// Computed cell dimensions (pixels) at current font_size.
    pub cell_width: f32,
    pub cell_height: f32,

    /// Current pixel dimensions (for resize after font change).
    pixel_width: f64,
    pixel_height: f64,

    /// Current terminal dimensions.
    pub cols: usize,
    pub rows: usize,

    /// Cursor blink state (toggled from main loop).
    pub cursor_visible: bool,
}

impl TerminalPane {
    /// Spawn a new terminal pane.
    ///
    /// `width` and `height` are the pixel dimensions of the terminal area.
    /// Returns `None` if PTY creation or shell spawn fails.
    pub fn spawn(width: f64, height: f64) -> Option<Self> {
        // Build font (Monaco preferred, Cascadia Code fallback) and compute cell dimensions.
        let font_data = load_terminal_font();
        let (cell_width, cell_height) = compute_cell_size(&font_data, DEFAULT_FONT_SIZE);

        let pad_px = cell_width as f64 * TERM_PAD_CELLS as f64;
        let usable_width = (width - pad_px * 2.0).max(0.0);
        let cols = ((usable_width / cell_width as f64).floor() as usize).max(2);
        let rows = ((height / cell_height as f64).floor() as usize).max(1);

        // Open a PTY pair.
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: rows as u16,
                cols: cols as u16,
                pixel_width: width as u16,
                pixel_height: height as u16,
            })
            .ok()?;

        // Spawn the shell.
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "bisque-computer");
        // Remove Claude Code env vars so `claude` can be launched inside the PTY
        // without it thinking it's already running inside Claude Code.
        cmd.env_remove("CLAUDECODE");
        cmd.env_remove("CLAUDE_CODE_ENTRYPOINT");

        let _child = pair.slave.spawn_command(cmd).ok()?;
        // `_child` is kept to ensure the process isn't reaped prematurely,
        // but we don't need to supervise it for the initial implementation.

        // Get read/write handles from the master.
        let reader = pair.master.try_clone_reader().ok()?;
        let writer = pair.master.take_writer().ok()?;

        // Spawn the PTY reader thread.
        let (tx, rx) = tokio_mpsc::unbounded_channel::<Vec<u8>>();
        {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut reader = reader;
                let mut buf = [0u8; 4096];
                loop {
                    match std::io::Read::read(&mut reader, &mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let _ = tx.send(buf[..n].to_vec());
                        }
                    }
                }
            });
        }

        // Build the alacritty_terminal Term.
        let size = TermSize::new(cols, rows);
        let term = Term::new(Config::default(), &size, TermEventListener);
        let term = Arc::new(Mutex::new(term));

        Some(Self {
            term,
            processor: Processor::new(),
            rx,
            pty_writer: writer,
            pty_resize: Box::new(LocalPtyResize(pair.master)),
            font: font_data,
            font_size: DEFAULT_FONT_SIZE,
            cell_width,
            cell_height,
            pixel_width: width,
            pixel_height: height,
            cols,
            rows,
            cursor_visible: true,
        })
    }

    /// Spawn a new terminal pane running a custom command (instead of $SHELL).
    ///
    /// Used by the sandboxed backend to run `sandbox-exec ... claude`.
    /// `width` and `height` are the pixel dimensions of the terminal area.
    pub fn spawn_command(
        width: f64,
        height: f64,
        cmd: &str,
        args: &[&str],
        env: &[(&str, &str)],
    ) -> Option<Self> {
        let font_data = load_terminal_font();
        let (cell_width, cell_height) = compute_cell_size(&font_data, DEFAULT_FONT_SIZE);

        let pad_px = cell_width as f64 * TERM_PAD_CELLS as f64;
        let usable_width = (width - pad_px * 2.0).max(0.0);
        let cols = ((usable_width / cell_width as f64).floor() as usize).max(2);
        let rows = ((height / cell_height as f64).floor() as usize).max(1);

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: rows as u16,
                cols: cols as u16,
                pixel_width: width as u16,
                pixel_height: height as u16,
            })
            .ok()?;

        let mut command = CommandBuilder::new(cmd);
        for arg in args {
            command.arg(*arg);
        }
        for (k, v) in env {
            command.env(*k, *v);
        }
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        command.env("TERM_PROGRAM", "bisque-computer");
        command.env_remove("CLAUDECODE");
        command.env_remove("CLAUDE_CODE_ENTRYPOINT");

        let _child = pair.slave.spawn_command(command).ok()?;

        let reader = pair.master.try_clone_reader().ok()?;
        let writer = pair.master.take_writer().ok()?;

        let (tx, rx) = tokio_mpsc::unbounded_channel::<Vec<u8>>();
        {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut reader = reader;
                let mut buf = [0u8; 4096];
                loop {
                    match std::io::Read::read(&mut reader, &mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let _ = tx.send(buf[..n].to_vec());
                        }
                    }
                }
            });
        }

        let size = TermSize::new(cols, rows);
        let term = Term::new(Config::default(), &size, TermEventListener);
        let term = Arc::new(Mutex::new(term));

        Some(Self {
            term,
            processor: Processor::new(),
            rx,
            pty_writer: writer,
            pty_resize: Box::new(LocalPtyResize(pair.master)),
            font: font_data,
            font_size: DEFAULT_FONT_SIZE,
            cell_width,
            cell_height,
            pixel_width: width,
            pixel_height: height,
            cols,
            rows,
            cursor_visible: true,
        })
    }

    /// Create a terminal pane from pre-wired I/O streams.
    ///
    /// Used by the VM backend: the caller provides the mpsc receiver (output from
    /// the VM), a writer (input to the VM), and a resize handle.
    pub fn from_streams(
        rx: tokio_mpsc::UnboundedReceiver<Vec<u8>>,
        writer: Box<dyn Write + Send>,
        resizer: Box<dyn PtyResize + Send>,
        cols: usize,
        rows: usize,
    ) -> Self {
        let font_data = load_terminal_font();
        let (cell_width, cell_height) = compute_cell_size(&font_data, DEFAULT_FONT_SIZE);

        let size = TermSize::new(cols, rows);
        let term = Term::new(Config::default(), &size, TermEventListener);
        let term = Arc::new(Mutex::new(term));

        Self {
            term,
            processor: Processor::new(),
            rx,
            pty_writer: writer,
            pty_resize: resizer,
            font: font_data,
            font_size: DEFAULT_FONT_SIZE,
            cell_width,
            cell_height,
            pixel_width: cols as f64 * cell_width as f64,
            pixel_height: rows as f64 * cell_height as f64,
            cols,
            rows,
            cursor_visible: true,
        }
    }

    /// Returns the terminal mono font data (Monaco preferred, Cascadia Code fallback).
    pub fn mono_font_data() -> FontData {
        load_terminal_font()
    }

    /// Drain all pending PTY output bytes from the channel and feed them to the
    /// terminal state machine.
    ///
    /// Must be called from the main thread (render loop) before each frame.
    pub fn drain_output(&mut self) {
        let mut collected: Vec<Vec<u8>> = Vec::new();
        while let Ok(chunk) = self.rx.try_recv() {
            collected.push(chunk);
        }
        if collected.is_empty() {
            return;
        }
        let mut term = self.term.lock().unwrap();
        for chunk in collected {
            self.processor.advance(&mut *term, &chunk);
        }
    }

    /// Resize the terminal to fit the given pixel area.
    ///
    /// Called when the window is resized or the terminal screen becomes active.
    pub fn resize(&mut self, width: f64, height: f64) {
        self.pixel_width = width;
        self.pixel_height = height;

        let pad_px = self.cell_width as f64 * TERM_PAD_CELLS as f64;
        let usable_width = (width - pad_px * 2.0).max(0.0);
        let new_cols = ((usable_width / self.cell_width as f64).floor() as usize).max(2);
        let new_rows = ((height / self.cell_height as f64).floor() as usize).max(1);

        if new_cols == self.cols && new_rows == self.rows {
            return;
        }

        self.cols = new_cols;
        self.rows = new_rows;

        // Resize the backend (PTY or VM serial console).
        self.pty_resize.resize(
            new_rows as u16,
            new_cols as u16,
            width as u16,
            height as u16,
        );

        // Resize the terminal grid.
        let new_size = TermSize::new(new_cols, new_rows);
        let mut term = self.term.lock().unwrap();
        term.resize(new_size);
    }

    /// Write a key event to the PTY.
    ///
    /// Should be called when the terminal screen is active and a key is pressed.
    /// Returns `true` if the key was consumed (written to the PTY).
    pub fn write_key(&mut self, key: &Key, ctrl_held: bool) -> bool {
        let bytes = key_event_to_pty_bytes(key, ctrl_held);
        if bytes.is_empty() {
            return false;
        }
        let _ = self.pty_writer.write_all(&bytes);
        let _ = self.pty_writer.flush();
        true
    }

    /// Increase font size by one step (Cmd+=).
    pub fn increase_font_size(&mut self) {
        self.set_font_size(self.font_size + FONT_SIZE_STEP);
    }

    /// Decrease font size by one step (Cmd+-).
    pub fn decrease_font_size(&mut self) {
        self.set_font_size(self.font_size - FONT_SIZE_STEP);
    }

    /// Reset font size to default (Cmd+0).
    pub fn reset_font_size(&mut self) {
        self.set_font_size(DEFAULT_FONT_SIZE);
    }

    /// Set font size, recompute cell dimensions, and resize the terminal grid.
    fn set_font_size(&mut self, size: f32) {
        let size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        if (size - self.font_size).abs() < 0.01 {
            return;
        }
        self.font_size = size;
        let (cw, ch) = compute_cell_size(&self.font, size);
        self.cell_width = cw;
        self.cell_height = ch;
        // Re-derive cols/rows from stored pixel dimensions and trigger PTY + grid resize.
        self.resize(self.pixel_width, self.pixel_height);
    }

    /// Render the terminal cell grid into a vello `Scene`.
    ///
    /// `offset_x` and `offset_y` are the top-left pixel position of the terminal
    /// area within the window.
    pub fn render_into_scene(&self, scene: &mut Scene, offset_x: f64, offset_y: f64, width: f64, height: f64) {
        // Background fill.
        let bg_rect = Rect::new(offset_x, offset_y, offset_x + width, offset_y + height);
        scene.fill(Fill::NonZero, Affine::IDENTITY, TERM_BG, None, &bg_rect);

        let term = self.term.lock().unwrap();
        let content = term.renderable_content();
        let colors = content.colors;

        let cw = self.cell_width as f64;
        let ch = self.cell_height as f64;

        // Horizontal padding (one cell width on each side).
        let pad_px = cw * TERM_PAD_CELLS as f64;
        let offset_x = offset_x + pad_px;

        // We'll accumulate glyphs for a single draw_glyphs call per color group,
        // but for correctness and simplicity we issue one call per glyph run.
        // In a future optimization pass, glyphs of the same color could be batched.
        let mut fg_glyphs: Vec<(Color, Vec<Glyph>)> = Vec::new();

        for cell in content.display_iter {
            let col = cell.point.column.0;
            let row = cell.point.line.0 as usize;

            // Skip cells outside the viewport.
            if col >= self.cols || row >= self.rows {
                continue;
            }

            let cell_x = offset_x + col as f64 * cw;
            let cell_y = offset_y + row as f64 * ch;

            // Compute background and foreground colors.
            let (bg_color, fg_color) = resolve_cell_colors(&cell.cell, colors);

            // Draw background if not the default terminal bg.
            let bg_default = TERM_BG;
            if bg_color != bg_default {
                let rect = Rect::new(cell_x, cell_y, cell_x + cw, cell_y + ch);
                scene.fill(Fill::NonZero, Affine::IDENTITY, bg_color, None, &rect);
            }

            // Skip wide-char spacers and empty cells.
            let ch_val = cell.cell.c;
            if ch_val == ' ' || ch_val == '\0' || cell.cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }

            // Resolve glyph ID.
            let font_ref = skrifa::FontRef::from_index(self.font.data.as_ref(), self.font.index);
            if let Ok(font_ref) = font_ref {
                use skrifa::MetadataProvider;
                let charmap = font_ref.charmap();
                let gid = charmap.map(ch_val).unwrap_or_default();
                let baseline_y = cell_y + ch as f64 * 0.8;
                let glyph = Glyph {
                    id: gid.to_u32(),
                    x: cell_x as f32,
                    y: baseline_y as f32,
                };
                // Batch with existing glyphs of the same color, or start new batch.
                if let Some(batch) = fg_glyphs.iter_mut().find(|(c, _)| *c == fg_color) {
                    batch.1.push(glyph);
                } else {
                    fg_glyphs.push((fg_color, vec![glyph]));
                }
            }
        }

        // Flush all glyph batches.
        for (color, glyphs) in fg_glyphs {
            scene
                .draw_glyphs(&self.font)
                .font_size(self.font_size)
                .brush(&color)
                .draw(Fill::NonZero, glyphs.into_iter());
        }

        // Draw cursor.
        if self.cursor_visible {
            let cursor = content.cursor;
            if cursor.shape != alacritty_terminal::vte::ansi::CursorShape::Hidden {
                let col = cursor.point.column.0;
                let row = {
                    // cursor.point.line is an i32 Line value relative to the scroll
                    // history. For the viewport we convert via display_offset.
                    let line_i32 = cursor.point.line.0;
                    let viewport_row = line_i32 + content.display_offset as i32;
                    if viewport_row < 0 || viewport_row >= self.rows as i32 {
                        // Cursor is outside the visible area.
                        return;
                    }
                    viewport_row as usize
                };
                if col < self.cols {
                    let cx = offset_x + col as f64 * cw;
                    let cy = offset_y + row as f64 * ch;
                    let cursor_rect = Rect::new(cx, cy, cx + cw, cy + ch);
                    scene.fill(Fill::NonZero, Affine::IDENTITY, TERM_CURSOR, None, &cursor_rect);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Color resolution
// ---------------------------------------------------------------------------

/// Resolve terminal cell fg/bg colors to vello `Color` values.
///
/// Handles `Named`, `Indexed`, and `Spec` (true-color) variants.
/// Falls back to sensible defaults for colors not present in the palette.
fn resolve_cell_colors(
    cell: &alacritty_terminal::term::cell::Cell,
    colors: &alacritty_terminal::term::color::Colors,
) -> (Color, Color) {
    let flags = cell.flags;
    let inverted = flags.contains(Flags::INVERSE);

    let mut fg = resolve_color(&cell.fg, colors, /* is_fg */ true);
    let mut bg = resolve_color(&cell.bg, colors, /* is_fg */ false);

    if inverted {
        std::mem::swap(&mut fg, &mut bg);
    }

    (bg, fg)
}

fn resolve_color(color: &AlaColor, colors: &alacritty_terminal::term::color::Colors, is_fg: bool) -> Color {
    match color {
        AlaColor::Spec(rgb) => rgb_to_color(*rgb),
        AlaColor::Named(named) => {
            // Try the palette first; fall back to hardcoded ANSI defaults.
            if let Some(rgb) = colors[*named] {
                return rgb_to_color(rgb);
            }
            named_color_fallback(*named, is_fg)
        }
        AlaColor::Indexed(idx) => {
            if let Some(rgb) = colors[*idx as usize] {
                return rgb_to_color(rgb);
            }
            // 256-color palette fallback.
            indexed_color_fallback(*idx)
        }
    }
}

fn rgb_to_color(rgb: Rgb) -> Color {
    Color::new([
        rgb.r as f32 / 255.0,
        rgb.g as f32 / 255.0,
        rgb.b as f32 / 255.0,
        1.0,
    ])
}

/// Hardcoded ANSI 16-color palette (xterm defaults).
fn named_color_fallback(named: NamedColor, is_fg: bool) -> Color {
    match named {
        NamedColor::Black => Color::new([0.0, 0.0, 0.0, 1.0]),
        NamedColor::Red => Color::new([0.80, 0.11, 0.11, 1.0]),
        NamedColor::Green => Color::new([0.13, 0.69, 0.30, 1.0]),
        NamedColor::Yellow => Color::new([0.80, 0.69, 0.11, 1.0]),
        NamedColor::Blue => Color::new([0.20, 0.40, 0.90, 1.0]),
        NamedColor::Magenta => Color::new([0.67, 0.11, 0.80, 1.0]),
        NamedColor::Cyan => Color::new([0.11, 0.69, 0.80, 1.0]),
        NamedColor::White => Color::new([0.75, 0.75, 0.75, 1.0]),
        NamedColor::BrightBlack => Color::new([0.25, 0.25, 0.25, 1.0]),
        NamedColor::BrightRed => Color::new([0.94, 0.30, 0.30, 1.0]),
        NamedColor::BrightGreen => Color::new([0.30, 0.87, 0.30, 1.0]),
        NamedColor::BrightYellow => Color::new([0.94, 0.94, 0.20, 1.0]),
        NamedColor::BrightBlue => Color::new([0.42, 0.59, 0.94, 1.0]),
        NamedColor::BrightMagenta => Color::new([0.87, 0.30, 0.94, 1.0]),
        NamedColor::BrightCyan => Color::new([0.30, 0.87, 0.94, 1.0]),
        NamedColor::BrightWhite => Color::new([0.94, 0.94, 0.94, 1.0]),
        NamedColor::Foreground => {
            if is_fg { TERM_FG } else { TERM_BG }
        }
        NamedColor::Background => {
            if is_fg { TERM_FG } else { TERM_BG }
        }
        NamedColor::Cursor => TERM_CURSOR,
        _ => if is_fg { TERM_FG } else { TERM_BG },
    }
}

/// Generate a color from the xterm 256-color cube for indices 16..=255.
fn indexed_color_fallback(idx: u8) -> Color {
    if idx < 16 {
        // Named colors range — shouldn't reach here normally.
        return TERM_FG;
    }
    if idx < 232 {
        // 6x6x6 color cube: indices 16..=231
        let idx = (idx - 16) as u32;
        let b = idx % 6;
        let g = (idx / 6) % 6;
        let r = idx / 36;
        let to_f = |v: u32| -> f32 { if v == 0 { 0.0 } else { (55 + v * 40) as f32 / 255.0 } };
        return Color::new([to_f(r), to_f(g), to_f(b), 1.0]);
    }
    // Grayscale ramp: indices 232..=255
    let level = (idx - 232) as f32;
    let v = (8.0 + level * 10.0) / 255.0;
    Color::new([v, v, v, 1.0])
}

// ---------------------------------------------------------------------------
// Key → PTY bytes
// ---------------------------------------------------------------------------

/// Convert a winit keyboard event to the byte sequence to send to the PTY.
///
/// Returns an empty `Vec` if the key has no terminal meaning.
pub fn key_event_to_pty_bytes(key: &Key, ctrl_held: bool) -> Vec<u8> {
    match key {
        Key::Character(c) => {
            let s = c.as_str();
            if ctrl_held {
                // Control characters: Ctrl+A..Z → \x01..\x1a
                if let Some(ch) = s.chars().next() {
                    let ch = ch.to_ascii_lowercase();
                    if ch >= 'a' && ch <= 'z' {
                        return vec![ch as u8 - b'a' + 1];
                    }
                    // Special ctrl sequences
                    match ch {
                        '[' => return vec![0x1b],     // Ctrl+[ = ESC
                        '\\' => return vec![0x1c],
                        ']' => return vec![0x1d],
                        '^' => return vec![0x1e],
                        '_' => return vec![0x1f],
                        ' ' => return vec![0x00],     // Ctrl+Space = NUL
                        _ => {}
                    }
                }
            }
            s.as_bytes().to_vec()
        }
        Key::Named(named) => match named {
            NamedKey::Enter => vec![b'\r'],
            NamedKey::Backspace => vec![0x7f],
            NamedKey::Tab => vec![b'\t'],
            NamedKey::Escape => vec![0x1b],
            NamedKey::Delete => b"\x1b[3~".to_vec(),
            // Arrow keys (normal cursor mode).
            NamedKey::ArrowUp => b"\x1b[A".to_vec(),
            NamedKey::ArrowDown => b"\x1b[B".to_vec(),
            NamedKey::ArrowRight => b"\x1b[C".to_vec(),
            NamedKey::ArrowLeft => b"\x1b[D".to_vec(),
            // Navigation keys.
            NamedKey::Home => b"\x1b[H".to_vec(),
            NamedKey::End => b"\x1b[F".to_vec(),
            NamedKey::PageUp => b"\x1b[5~".to_vec(),
            NamedKey::PageDown => b"\x1b[6~".to_vec(),
            // Space (winit may emit space as Named(Space) instead of Character(" ")).
            NamedKey::Space => vec![b' '],
            // Insert key.
            NamedKey::Insert => b"\x1b[2~".to_vec(),
            // Function keys F1-F12 (standard xterm sequences).
            NamedKey::F1 => b"\x1bOP".to_vec(),
            NamedKey::F2 => b"\x1bOQ".to_vec(),
            NamedKey::F3 => b"\x1bOR".to_vec(),
            NamedKey::F4 => b"\x1bOS".to_vec(),
            NamedKey::F5 => b"\x1b[15~".to_vec(),
            NamedKey::F6 => b"\x1b[17~".to_vec(),
            NamedKey::F7 => b"\x1b[18~".to_vec(),
            NamedKey::F8 => b"\x1b[19~".to_vec(),
            NamedKey::F9 => b"\x1b[20~".to_vec(),
            NamedKey::F10 => b"\x1b[21~".to_vec(),
            NamedKey::F11 => b"\x1b[23~".to_vec(),
            NamedKey::F12 => b"\x1b[24~".to_vec(),
            _ => vec![],
        },
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Font metrics helper
// ---------------------------------------------------------------------------

/// Compute cell dimensions for a monospace font at the given size.
///
/// Returns `(cell_width, cell_height)` in pixels.
fn compute_cell_size(font_data: &FontData, font_size: f32) -> (f32, f32) {
    let font_ref = skrifa::FontRef::from_index(font_data.data.as_ref(), font_data.index);
    if let Ok(font_ref) = font_ref {
        use skrifa::MetadataProvider;
        let charmap = font_ref.charmap();
        let metrics = font_ref.glyph_metrics(
            skrifa::instance::Size::new(font_size),
            skrifa::instance::LocationRef::default(),
        );
        let m_gid = charmap.map('M').unwrap_or_default();
        let advance = metrics.advance_width(m_gid).unwrap_or(font_size * 0.6);
        // Line height: use units_per_em-derived estimate (approx 1.2× font size).
        let line_height = font_size * 1.4;
        return (advance, line_height);
    }
    // Fallback: rough monospace estimates.
    (font_size * 0.6, font_size * 1.4)
}
