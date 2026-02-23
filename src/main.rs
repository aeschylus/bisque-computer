//! bisque-computer: Lobster Instance Dashboard
//!
//! A GPU-rendered fullscreen dashboard that connects to one or more Lobster
//! Dashboard WebSocket servers and visualizes system state in real time.
//!
//! Uses vello/wgpu for rendering and tokio-tungstenite for WebSocket communication.
//! Voice input (Shift+Enter push-to-talk) transcribes audio via whisper.cpp and
//! sends the result as a voice_input message over the existing WebSocket connection.
//!
//! On first launch (no config file), shows a setup screen where the user enters
//! the connection URL. The URL is saved to `~/.config/bisque-computer/server`.
//!
//! ## Multi-screen layout
//!
//! Three screens are arranged horizontally:
//! - Screen 0 (Dashboard): existing dashboard
//! - Screen 1 (Info): memory events + active agents
//! - Screen 2 (Terminal): PTY-backed terminal emulator
//!
//! Cmd+Right / Cmd+Left slides between screens with a spring animation.
//!
//! ## State Management
//!
//! All app-level state is modelled with `statig` hierarchical state machines
//! defined in `state_machine.rs`:
//!
//! - `VoiceMachine`: voice input lifecycle (disabled <-> idle <-> recording <->
//!   transcribing <-> done/error)
//! - `AppModeMachine`: display mode (setup -> dashboard)
//!
//! Mouse events for text selection are routed through `text_selection::SelectableText`
//! instances, backed by `parley::PlainEditor`.

mod dashboard;
mod design;
mod info_screen;
mod logging;
#[allow(dead_code)]
mod protocol;
mod state_machine;
mod terminal;
mod text_selection;
mod token_watcher;
mod voice;
mod ws_client;

use anyhow::Result;
use clap::Parser;
use statig::prelude::*;
use statig::blocking::StateMachine;
use tracing::{error, info, warn};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use vello::kurbo::Affine;
use vello::peniko::color::palette;
use vello::peniko::FontData;
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, KeyEvent, Modifiers, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window};

use vello::wgpu;

use state_machine::{AppModeEvent, AppModeMachine, VoiceEvent, VoiceMachine};
use state_machine::voice_sm::State as VoiceState;
use state_machine::app_mode_sm::State as AppModeState;
use design::DesignTokens;
use terminal::TerminalPane;
use text_selection::{ParleyCtx, SelectableText};

// ---------------------------------------------------------------------------
// Config file helpers
// ---------------------------------------------------------------------------

/// Path to the server URL config file: `~/.config/bisque-computer/server`.
fn config_file_path() -> PathBuf {
    let mut p = config_base_dir();
    p.push("bisque-computer");
    p.push("server");
    p
}

/// Return the OS-appropriate config base directory.
fn config_base_dir() -> PathBuf {
    if let Ok(val) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(val);
    }
    #[cfg(target_os = "macos")]
    {
        let mut p = home_dir();
        p.push("Library");
        p.push("Application Support");
        return p;
    }
    #[allow(unreachable_code)]
    {
        let mut p = home_dir();
        p.push(".config");
        p
    }
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

fn read_saved_server_url() -> Option<String> {
    let path = config_file_path();
    if !path.exists() {
        return None;
    }
    let url = std::fs::read_to_string(&path).ok()?.trim().to_string();
    if url.is_empty() { None } else { Some(url) }
}

fn save_server_url(url: &str) -> Result<()> {
    let path = config_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, url)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Lobster Instance Dashboard
#[derive(Parser, Debug)]
#[command(name = "bisque-computer", version, about = "Lobster Instance Dashboard")]
struct Args {
    /// WebSocket endpoint URLs to connect to (e.g., ws://localhost:9100)
    #[arg(short, long, default_value = "ws://localhost:9100")]
    endpoints: Vec<String>,

    /// Start in windowed mode instead of fullscreen
    #[arg(short, long)]
    windowed: bool,

    /// Disable voice input
    #[arg(long)]
    no_voice: bool,
}

// ---------------------------------------------------------------------------
// Screen index
// ---------------------------------------------------------------------------

/// The three horizontally-arranged screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScreenIndex {
    Dashboard = 0,
    Info = 1,
    Terminal = 2,
}

impl ScreenIndex {
    fn from_usize(n: usize) -> Self {
        match n {
            0 => ScreenIndex::Dashboard,
            1 => ScreenIndex::Info,
            2 => ScreenIndex::Terminal,
            _ => ScreenIndex::Dashboard,
        }
    }

    fn to_usize(self) -> usize {
        self as usize
    }

    const COUNT: usize = 3;
}

// ---------------------------------------------------------------------------
// Render surface lifecycle
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum RenderState {
    Active {
        surface: Box<RenderSurface<'static>>,
        valid_surface: bool,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Cursor blink interval in milliseconds.
const CURSOR_BLINK_MS: u64 = 500;

/// Spring constant for screen slide animation.
const SPRING_K: f64 = 0.2;

/// Snap threshold: stop animating when within this many screen-widths of target.
const SNAP_THRESHOLD: f64 = 0.001;

struct App {
    // --- Design tokens ---
    tokens: Arc<RwLock<DesignTokens>>,

    // --- Rendering infrastructure ---
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    render_state: RenderState,
    /// Root scene composed from per-screen scenes.
    scene: Scene,
    start_time: Instant,
    windowed: bool,
    font_data: Option<FontData>,
    #[allow(dead_code)]
    mono_font_data: Option<FontData>,

    // --- State machines ---
    voice_machine: StateMachine<VoiceMachine>,
    app_mode_machine: StateMachine<AppModeMachine>,

    // --- Input ---
    modifiers: Modifiers,
    cursor_pos: PhysicalPosition<f64>,

    // --- Text selection ---
    parley_ctx: ParleyCtx,
    selectable_regions: Vec<SelectableText>,

    // --- Setup mode ---
    setup_input: String,

    // --- Multi-screen animation ---
    /// Current screen (integer target).
    current_screen: ScreenIndex,
    /// Animated horizontal offset in screen-widths (0.0 = leftmost screen visible).
    screen_offset: f64,
    /// Target offset (updated immediately on Cmd+Arrow).
    target_offset: f64,
    /// Whether we are currently animating between screens.
    animating: bool,
    /// Timestamp of the last redraw (for animation delta-time, if needed).
    last_frame: Instant,

    // --- Terminal ---
    terminal: Option<TerminalPane>,
    /// Last cursor blink instant.
    last_blink: Instant,
}

impl App {
    fn is_dashboard(&self) -> bool {
        matches!(self.app_mode_machine.state(), AppModeState::Dashboard {})
    }

    fn voice_enabled(&self) -> bool {
        !matches!(self.voice_machine.state(), VoiceState::Disabled {})
    }

    fn handle_voice_event(&mut self, event: VoiceEvent) {
        self.voice_machine.handle(&event);
    }

    fn poll_transcription(&mut self) {
        let maybe_event = self.voice_machine.inner().poll_transcription();
        if let Some(event) = maybe_event {
            self.voice_machine.handle(&event);
        }
    }

    fn copy_selection_to_clipboard(&self) {
        for region in &self.selectable_regions {
            if let Some(text) = region.selected_text() {
                if !text.is_empty() {
                    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.to_string())) {
                        Ok(()) => info!(target: "selection", "Copied {} chars to clipboard", text.len()),
                        Err(e) => error!(target: "selection", "Clipboard write failed: {}", e),
                    }
                    return;
                }
            }
        }
    }

    fn rebuild_selectable_regions(&mut self, width: f64) {
        self.selectable_regions.clear();

        let instances = {
            let guard = match self.app_mode_machine.inner().instances.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let v: Vec<crate::protocol::LobsterInstance> = guard.clone();
            v
        };

        for (i, inst) in instances.iter().enumerate() {
            let text = format!(
                "{} — {}",
                inst.url,
                match &inst.status {
                    crate::protocol::ConnectionStatus::Connected =>
                        inst.state.system.hostname.clone(),
                    crate::protocol::ConnectionStatus::Connecting =>
                        "connecting...".to_string(),
                    crate::protocol::ConnectionStatus::Disconnected =>
                        "disconnected".to_string(),
                    crate::protocol::ConnectionStatus::Error(e) =>
                        format!("error: {}", &e[..e.len().min(40)]),
                }
            );

            let origin = (24.0, 80.0 + i as f64 * 60.0);
            let max_w = (width - 48.0) as f32;

            let region = SelectableText::new(
                &text,
                28.0,
                origin,
                Some(max_w),
                &mut self.parley_ctx,
            );
            self.selectable_regions.push(region);
        }
    }

    /// Navigate to the next screen (Cmd+Right).
    fn navigate_right(&mut self) {
        let next = (self.current_screen.to_usize() + 1).min(ScreenIndex::COUNT - 1);
        self.current_screen = ScreenIndex::from_usize(next);
        self.target_offset = next as f64;
        self.animating = true;
    }

    /// Navigate to the previous screen (Cmd+Left).
    fn navigate_left(&mut self) {
        let prev = self.current_screen.to_usize().saturating_sub(1);
        self.current_screen = ScreenIndex::from_usize(prev);
        self.target_offset = prev as f64;
        self.animating = true;
    }

    /// Advance the spring animation one step.
    ///
    /// Returns `true` if animation is still in progress (caller should request
    /// another redraw).
    fn tick_animation(&mut self) -> bool {
        if !self.animating {
            return false;
        }
        let tokens = self.tokens.read().unwrap();
        let delta = self.target_offset - self.screen_offset;
        if delta.abs() < tokens.animation.snap_threshold {
            self.screen_offset = self.target_offset;
            self.animating = false;
            return false;
        }
        self.screen_offset += delta * tokens.animation.spring_k;
        true
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.render_state else {
            return;
        };

        let window = cached_window
            .take()
            .unwrap_or_else(|| create_window(event_loop, self.windowed));

        let size = window.inner_size();
        let surface_future = self.context.create_surface(
            window.clone(),
            size.width,
            size.height,
            wgpu::PresentMode::AutoVsync,
        );
        let surface = pollster::block_on(surface_future).expect("Error creating surface");

        self.renderers
            .resize_with(self.context.devices.len(), || None);
        self.renderers[surface.dev_id]
            .get_or_insert_with(|| create_renderer(&self.context, &surface));

        self.render_state = RenderState::Active {
            surface: Box::new(surface),
            valid_surface: true,
            window,
        };
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let RenderState::Active { window, .. } = &self.render_state {
            self.render_state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let is_our_window = match &self.render_state {
            RenderState::Active { window, .. } => window.id() == window_id,
            _ => false,
        };
        if !is_our_window {
            return;
        }

        match event {
            // ----------------------------------------------------------------
            // Window lifecycle
            // ----------------------------------------------------------------
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                if let RenderState::Active {
                    surface,
                    valid_surface,
                    ..
                } = &mut self.render_state
                {
                    if size.width != 0 && size.height != 0 {
                        self.context.resize_surface(surface, size.width, size.height);
                        *valid_surface = true;
                        // Notify terminal of new size.
                        if let Some(term) = &mut self.terminal {
                            term.resize(size.width as f64, size.height as f64);
                        }
                    } else {
                        *valid_surface = false;
                    }
                }
            }

            // ----------------------------------------------------------------
            // Modifier tracking
            // ----------------------------------------------------------------
            WindowEvent::ModifiersChanged(new_mods) => {
                let shift_was_held = self.modifiers.state().shift_key();
                self.modifiers = new_mods;
                let shift_now_held = self.modifiers.state().shift_key();

                if self.is_dashboard()
                    && shift_was_held
                    && !shift_now_held
                    && matches!(self.voice_machine.state(), VoiceState::Recording {})
                {
                    self.handle_voice_event(VoiceEvent::StopRecording);
                }
            }

            // ----------------------------------------------------------------
            // Global hotkeys
            // ----------------------------------------------------------------
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if self.current_screen != ScreenIndex::Terminal => event_loop.exit(),

            // Escape in terminal: send ESC to PTY (handled below in the terminal
            // keyboard routing block).

            // ----------------------------------------------------------------
            // Multi-screen navigation: Cmd+Right / Cmd+Left
            // ----------------------------------------------------------------
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::ArrowRight),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if self.modifiers.state().super_key() => {
                self.navigate_right();
                if let RenderState::Active { window, .. } = &self.render_state {
                    window.request_redraw();
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::ArrowLeft),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if self.modifiers.state().super_key() => {
                self.navigate_left();
                if let RenderState::Active { window, .. } = &self.render_state {
                    window.request_redraw();
                }
            }

            // ----------------------------------------------------------------
            // Terminal screen: keyboard → PTY
            // ----------------------------------------------------------------
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        ref logical_key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if self.current_screen == ScreenIndex::Terminal => {
                // Cmd+Left/Right are consumed above, so anything reaching here
                // that is NOT a Cmd+Arrow goes to the PTY.
                if !self.modifiers.state().super_key() {
                    let ctrl = self.modifiers.state().control_key();
                    if let Some(term) = &mut self.terminal {
                        term.write_key(logical_key, ctrl);
                    }
                }
            }

            // ----------------------------------------------------------------
            // Setup mode: URL input
            // ----------------------------------------------------------------
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Backspace),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if matches!(self.app_mode_machine.state(), AppModeState::Setup {}) => {
                self.setup_input.pop();
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Enter),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if matches!(self.app_mode_machine.state(), AppModeState::Setup {}) => {
                let url = self.setup_input.trim().to_string();
                if url.starts_with("ws://") || url.starts_with("wss://") {
                    match save_server_url(&url) {
                        Ok(()) => {
                            info!(target: "setup", "Saved server URL: {}", url);

                            if let Ok(parsed) = url::Url::parse(&url) {
                                if let Some(h) = parsed.host_str() {
                                    // SAFETY: we are not inside a statig state handler.
                                    unsafe {
                                        self.voice_machine.inner_mut().config.whisper_host =
                                            h.to_string();
                                    }
                                    info!(target: "setup", "whisper host set to: {}", h);
                                }
                            }

                            // SAFETY: we are not inside a statig state handler.
                            unsafe {
                                self.app_mode_machine.inner_mut().setup_input = url.clone();
                            }
                            self.app_mode_machine.handle(&AppModeEvent::UrlSubmitted(url));
                        }
                        Err(e) => error!(target: "setup", "Failed to save server URL: {}", e),
                    }
                } else {
                    warn!(target: "setup", "URL must start with ws:// or wss://");
                }
            }

            // Cmd+V / Ctrl+V paste in setup mode.
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Character(ref c),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if matches!(self.app_mode_machine.state(), AppModeState::Setup {})
                && (c.as_str() == "v" || c.as_str() == "V")
                && (self.modifiers.state().super_key()
                    || self.modifiers.state().control_key()) =>
            {
                match arboard::Clipboard::new().and_then(|mut cb| cb.get_text()) {
                    Ok(text) => {
                        self.setup_input.push_str(&text);
                        info!(target: "setup", "Pasted {} chars from clipboard", text.len());
                    }
                    Err(e) => error!(target: "setup", "Clipboard read failed: {}", e),
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Character(ref c),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if matches!(self.app_mode_machine.state(), AppModeState::Setup {}) => {
                self.setup_input.push_str(c.as_str());
            }

            // ----------------------------------------------------------------
            // Dashboard mode: voice and clipboard
            // ----------------------------------------------------------------

            // Shift+Enter: push-to-talk.
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Enter),
                        state,
                        repeat: false,
                        ..
                    },
                ..
            } if self.modifiers.state().shift_key() && self.is_dashboard()
                && self.current_screen == ScreenIndex::Dashboard =>
            {
                match state {
                    ElementState::Pressed => {
                        self.handle_voice_event(VoiceEvent::ShiftEnterPressed);
                    }
                    ElementState::Released => {
                        if matches!(self.voice_machine.state(), VoiceState::Recording {}) {
                            self.handle_voice_event(VoiceEvent::StopRecording);
                        }
                    }
                }
            }

            // 'V' key: toggle voice (dashboard screen only).
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Character(ref c),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if (c.as_str() == "v" || c.as_str() == "V")
                && self.is_dashboard()
                && self.current_screen == ScreenIndex::Dashboard
                && !self.modifiers.state().super_key()
                && !self.modifiers.state().control_key() =>
            {
                if self.voice_enabled() {
                    self.handle_voice_event(VoiceEvent::Disable);
                    info!(target: "voice", "Voice input disabled");
                } else {
                    self.handle_voice_event(VoiceEvent::Enable);
                    info!(target: "voice", "Voice input enabled");
                }
            }

            // Cmd+C / Ctrl+C: copy selected text (non-terminal screens).
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Character(ref c),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if (c.as_str() == "c" || c.as_str() == "C")
                && (self.modifiers.state().super_key()
                    || self.modifiers.state().control_key())
                && self.current_screen != ScreenIndex::Terminal =>
            {
                self.copy_selection_to_clipboard();
            }

            // 'R' key: reserved.
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Character(ref c),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if (c.as_str() == "r" || c.as_str() == "R")
                && self.is_dashboard()
                && self.current_screen == ScreenIndex::Dashboard => {}

            // ----------------------------------------------------------------
            // Mouse: text selection (dashboard screen only)
            // ----------------------------------------------------------------
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = position;
                if self.is_dashboard() && self.current_screen == ScreenIndex::Dashboard {
                    let x = position.x;
                    let y = position.y;
                    let parley_ctx = &mut self.parley_ctx;
                    for region in &mut self.selectable_regions {
                        region.handle_mouse_drag(x, y, parley_ctx);
                    }
                }
            }

            WindowEvent::MouseInput {
                state: btn_state,
                button: MouseButton::Left,
                ..
            } if self.is_dashboard() && self.current_screen == ScreenIndex::Dashboard => {
                let x = self.cursor_pos.x;
                let y = self.cursor_pos.y;
                match btn_state {
                    ElementState::Pressed => {
                        let parley_ctx = &mut self.parley_ctx;
                        for region in &mut self.selectable_regions {
                            if region.hit_test(x, y) {
                                region.handle_mouse_press(x, y, parley_ctx);
                            }
                        }
                    }
                    ElementState::Released => {
                        for region in &mut self.selectable_regions {
                            region.handle_mouse_release();
                        }
                    }
                }
            }

            // ----------------------------------------------------------------
            // Render
            // ----------------------------------------------------------------
            WindowEvent::RedrawRequested => {
                self.poll_transcription();

                // Drain PTY output before rendering.
                if let Some(term) = &mut self.terminal {
                    term.drain_output();
                }

                // Acquire design tokens for this frame.
                let tokens = self.tokens.read().unwrap();

                // Cursor blink.
                let blink_elapsed = self.last_blink.elapsed();
                if blink_elapsed >= std::time::Duration::from_millis(tokens.animation.cursor_blink_ms) {
                    if let Some(term) = &mut self.terminal {
                        term.cursor_visible = !term.cursor_visible;
                    }
                    self.last_blink = Instant::now();
                }

                // Compute state snapshots before borrowing the render surface.
                let voice_ui_state = VoiceMachine::ui_state(self.voice_machine.state());
                let voice_enabled = !matches!(
                    self.voice_machine.state(),
                    VoiceState::Disabled {}
                );
                let is_setup = matches!(
                    self.app_mode_machine.state(),
                    AppModeState::Setup {}
                );

                let RenderState::Active {
                    surface,
                    valid_surface: true,
                    window,
                } = &mut self.render_state
                else {
                    return;
                };

                // Clone the window Arc early so we can use it after releasing the
                // &mut self.render_state borrow (needed for tick_animation()).
                let window_arc = window.clone();

                self.scene.reset();

                let width = surface.config.width as f64;
                let height = surface.config.height as f64;
                let elapsed = self.start_time.elapsed().as_secs_f64();

                if is_setup {
                    // In setup mode: render setup screen directly (no multi-screen).
                    dashboard::render_setup_screen(
                        &mut self.scene,
                        width,
                        height,
                        &self.setup_input,
                        self.font_data.as_ref(),
                    );
                } else {
                    // --- Multi-screen compositing ---
                    //
                    // Each screen is rendered into a separate Scene, then composited
                    // into the root scene with a horizontal Affine::translate.
                    //
                    // The offset is in screen-widths:
                    //   screen_offset = 0.0 → dashboard fully visible (screen 0 at x=0)
                    //   screen_offset = 1.0 → info screen fully visible (screen 1 at x=0)
                    //   screen_offset = 2.0 → terminal fully visible (screen 2 at x=0)

                    let offset_px = self.screen_offset * width;

                    // --- Screen 0: Dashboard ---
                    let mut dashboard_scene = Scene::new();
                    {
                        let instances = &self.app_mode_machine.inner().instances;
                        dashboard::render_dashboard(
                            &mut dashboard_scene,
                            width,
                            height,
                            instances,
                            elapsed,
                            self.font_data.as_ref(),
                            &voice_ui_state,
                            voice_enabled,
                            &tokens,
                        );
                        // Render text selection overlays.
                        let selection_color = vello::peniko::Color::new([0.2_f32, 0.5, 1.0, 0.35]);
                        let text_color = vello::peniko::Color::new([0.0_f32, 0.0, 0.0, 1.0]);
                        let parley_ctx = &mut self.parley_ctx;
                        for region in &mut self.selectable_regions {
                            region.render_into_scene(
                                &mut dashboard_scene,
                                text_color,
                                selection_color,
                                parley_ctx,
                            );
                        }
                    }
                    self.scene.append(
                        &dashboard_scene,
                        Some(Affine::translate((0.0 * width - offset_px, 0.0))),
                    );

                    // --- Screen 1: Info ---
                    let mut info_scene = Scene::new();
                    {
                        let instances = &self.app_mode_machine.inner().instances;
                        info_screen::render_info_screen(
                            &mut info_scene,
                            width,
                            height,
                            instances,
                            self.font_data.as_ref(),
                            &tokens,
                        );
                    }
                    self.scene.append(
                        &info_scene,
                        Some(Affine::translate((1.0 * width - offset_px, 0.0))),
                    );

                    // --- Screen 2: Terminal ---
                    let mut terminal_scene = Scene::new();
                    if let Some(term) = &self.terminal {
                        term.render_into_scene(&mut terminal_scene, 0.0, 0.0, width, height, &tokens);
                    } else {
                        // Terminal not yet spawned; show a placeholder.
                        render_terminal_placeholder(&mut terminal_scene, width, height, self.font_data.as_ref());
                    }
                    self.scene.append(
                        &terminal_scene,
                        Some(Affine::translate((2.0 * width - offset_px, 0.0))),
                    );
                }

                // Release the design tokens read lock before GPU submission.
                drop(tokens);

                let device_handle = &self.context.devices[surface.dev_id];

                self.renderers[surface.dev_id]
                    .as_mut()
                    .unwrap()
                    .render_to_texture(
                        &device_handle.device,
                        &device_handle.queue,
                        &self.scene,
                        &surface.target_view,
                        &vello::RenderParams {
                            base_color: palette::css::BISQUE,
                            width: surface.config.width,
                            height: surface.config.height,
                            antialiasing_method: AaConfig::Msaa16,
                        },
                    )
                    .expect("failed to render to surface");

                let surface_texture = surface
                    .surface
                    .get_current_texture()
                    .expect("failed to get surface texture");

                let mut encoder =
                    device_handle
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Surface Blit"),
                        });
                surface.blitter.copy(
                    &device_handle.device,
                    &mut encoder,
                    &surface.target_view,
                    &surface_texture
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default()),
                );
                device_handle.queue.submit([encoder.finish()]);
                surface_texture.present();
                device_handle.device.poll(wgpu::PollType::Poll).unwrap();

                // Advance the spring animation.
                // `surface` and `window` are no longer used after this point.
                // The NLL borrow checker allows tick_animation() here because the
                // mutable borrow of self.render_state doesn't extend past the last use.
                let still_animating = self.tick_animation();
                if still_animating {
                    window_arc.request_redraw();
                } else {
                    // Schedule cursor blink wakeup via WaitUntil.
                    let blink_ms = self.tokens.read().unwrap().animation.cursor_blink_ms;
                    let next_blink = self.last_blink
                        + std::time::Duration::from_millis(blink_ms);
                    event_loop.set_control_flow(ControlFlow::WaitUntil(next_blink));
                    window_arc.request_redraw();
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Triggered by WaitUntil expiry (cursor blink) or external wakeup.
        // Re-request a redraw so the terminal cursor blinks.
        if let RenderState::Active { window, .. } = &self.render_state {
            window.request_redraw();
        }
    }
}

// ---------------------------------------------------------------------------
// Terminal placeholder (shown before terminal is spawned)
// ---------------------------------------------------------------------------

fn render_terminal_placeholder(scene: &mut Scene, width: f64, height: f64, font_data: Option<&FontData>) {
    use vello::kurbo::Rect;
    use vello::peniko::{Color, Fill};
    use vello::kurbo::Affine;
    let bg = Color::new([0.08, 0.08, 0.10, 1.0]);
    scene.fill(Fill::NonZero, Affine::IDENTITY, bg, None, &Rect::new(0.0, 0.0, width, height));
    let fg = Color::new([0.85, 0.85, 0.90, 1.0]);
    dashboard::draw_centered_text_pub(scene, width, height, "Terminal (loading...)", fg, 32.0, font_data);
}

fn main() -> Result<()> {
    let args = Args::parse();

    let _log_guard = logging::init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    let rt_handle = runtime.handle().clone();

    info!("bisque-computer v{}", env!("CARGO_PKG_VERSION"));

    let explicit_endpoints =
        args.endpoints.len() != 1 || args.endpoints[0] != "ws://localhost:9100";

    let (start_in_setup, endpoints) = if explicit_endpoints {
        (false, args.endpoints.clone())
    } else if let Some(saved_url) = read_saved_server_url() {
        info!("Using saved server URL: {}", saved_url);
        (false, vec![saved_url])
    } else {
        info!("No server URL configured — starting setup screen.");
        (true, vec!["ws://setup-placeholder:9100".to_string()])
    };

    if !start_in_setup {
        info!("Connecting to {} endpoint(s):", endpoints.len());
        for ep in &endpoints {
            info!("  - {}", ep);
        }
    }

    let mut voice_config = voice::VoiceConfig::default();
    if args.no_voice {
        voice_config.enabled = false;
    }
    if !start_in_setup && !endpoints.is_empty() {
        if let Ok(parsed) = url::Url::parse(&endpoints[0]) {
            if let Some(h) = parsed.host_str() {
                voice_config.whisper_host = h.to_string();
            }
        }
    }

    let (instances, outbound) = ws_client::spawn_clients(&runtime, endpoints);

    let font_data = dashboard::load_readable_font();
    let mono_font_data = dashboard::load_mono_font();

    if font_data.is_some() {
        info!("Loaded readable font (Optima/Helvetica/DejaVu Sans)");
    } else {
        warn!("No system font found — using bitmap fallback");
    }

    if voice_config.enabled {
        info!(
            whisper_host = %voice_config.whisper_host,
            whisper_port = voice_config.whisper_port,
            "Voice input: enabled (Shift+Enter to record, V to toggle)"
        );
    } else {
        info!("Voice input: disabled (--no-voice flag set)");
    }

    // Build voice state machine.
    let voice_sm_base = VoiceMachine::new(voice_config.clone(), rt_handle, outbound.clone());
    let voice_machine = if !voice_config.enabled {
        let mut sm = voice_sm_base.uninitialized_state_machine();
        *sm.state_mut() = VoiceState::Disabled {};
        let initialized = sm.init();
        initialized.into()
    } else {
        let mut sm = voice_sm_base.state_machine();
        sm.init();
        sm
    };

    // Build app-mode state machine.
    let app_mode_base = AppModeMachine::new(instances.clone(), outbound.clone());
    let app_mode_machine = if start_in_setup {
        let mut sm = app_mode_base.state_machine();
        sm.init();
        sm
    } else {
        let mut sm = app_mode_base.uninitialized_state_machine();
        *sm.state_mut() = AppModeState::Dashboard {};
        let initialized = sm.init();
        initialized.into()
    };

    // Spawn terminal pane.
    // Use a reasonable default size; will be resized when window is available.
    let terminal = TerminalPane::spawn(1280.0, 800.0);
    if terminal.is_some() {
        info!("Terminal: spawned PTY-backed terminal (Cascadia Code embedded)");
    } else {
        warn!("Terminal: failed to spawn PTY — terminal screen will show placeholder");
    }

    // TODO(layer-3): Wire token watcher into App
    // let tokens = Arc::new(RwLock::new(DesignTokens::default()));
    // let tokens_for_watcher = tokens.clone();
    // let watcher = token_watcher::TokenFileWatcher::start(move |toml_content| {
    //     match DesignTokens::from_toml(&toml_content) {
    //         Ok(new_tokens) => {
    //             if let Ok(mut t) = tokens_for_watcher.write() {
    //                 *t = new_tokens;
    //             }
    //             // proxy.send_event(UserEvent::TokensChanged).ok();
    //         }
    //         Err(e) => eprintln!("design.toml parse error (keeping previous): {e}"),
    //     }
    // }).expect("Failed to start token file watcher");
    // info!("Watching design tokens at: {}", watcher.path().display());

    let now = Instant::now();

    let mut app = App {
        tokens: Arc::new(RwLock::new(DesignTokens::default())),
        context: RenderContext::new(),
        renderers: vec![],
        render_state: RenderState::Suspended(None),
        scene: Scene::new(),
        start_time: now,
        windowed: args.windowed,
        font_data,
        mono_font_data,
        voice_machine,
        app_mode_machine,
        modifiers: Modifiers::default(),
        cursor_pos: PhysicalPosition::default(),
        parley_ctx: ParleyCtx::new(),
        selectable_regions: Vec::new(),
        setup_input: String::new(),
        current_screen: ScreenIndex::Dashboard,
        screen_offset: 0.0,
        target_offset: 0.0,
        animating: false,
        last_frame: now,
        terminal,
        last_blink: now,
    };

    let event_loop = EventLoop::new()?;
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");

    runtime.shutdown_timeout(std::time::Duration::from_secs(1));

    Ok(())
}

fn create_window(event_loop: &ActiveEventLoop, windowed: bool) -> Arc<Window> {
    let mut attr = Window::default_attributes().with_title("bisque-computer | Lobster Dashboard");

    if !windowed {
        attr = attr.with_fullscreen(Some(Fullscreen::Borderless(None)));
    } else {
        attr = attr.with_inner_size(winit::dpi::LogicalSize::new(1280, 800));
    }

    Arc::new(event_loop.create_window(attr).unwrap())
}

fn create_renderer(render_cx: &RenderContext, surface: &RenderSurface<'_>) -> Renderer {
    Renderer::new(
        &render_cx.devices[surface.dev_id].device,
        RendererOptions::default(),
    )
    .expect("Couldn't create renderer")
}
