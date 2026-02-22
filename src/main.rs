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

mod dashboard;
#[allow(dead_code)]
mod protocol;
mod voice;
mod ws_client;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use vello::peniko::color::palette;
use vello::peniko::FontData;
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, Modifiers, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window};

use vello::wgpu;

use voice::{VoiceConfig, VoiceUiState};
use ws_client::{OutboundSender, SharedInstances};

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
    // Respect XDG_CONFIG_HOME if set (Linux/CI)
    if let Ok(val) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(val);
    }
    // macOS: ~/Library/Application Support
    #[cfg(target_os = "macos")]
    {
        let mut p = home_dir();
        p.push("Library");
        p.push("Application Support");
        return p;
    }
    // Linux and others: ~/.config
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

/// Read the saved server URL from `~/.config/bisque-computer/server`.
/// Returns `None` when the file is absent or empty.
fn read_saved_server_url() -> Option<String> {
    let path = config_file_path();
    if !path.exists() {
        return None;
    }
    let url = std::fs::read_to_string(&path).ok()?.trim().to_string();
    if url.is_empty() { None } else { Some(url) }
}

/// Persist `url` to `~/.config/bisque-computer/server`, creating dirs as needed.
fn save_server_url(url: &str) -> Result<()> {
    let path = config_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, url)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Application mode
// ---------------------------------------------------------------------------

/// Determines which screen is currently active.
#[derive(Debug, PartialEq, Eq)]
enum AppMode {
    /// Normal dashboard: connected (or connecting) to a Lobster server.
    Dashboard,
    /// First-run setup: waiting for the user to paste a connection URL.
    Setup,
}

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

#[derive(Debug)]
enum RenderState {
    Active {
        surface: Box<RenderSurface<'static>>,
        valid_surface: bool,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
}

struct App {
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    state: RenderState,
    scene: Scene,
    start_time: Instant,
    instances: SharedInstances,
    outbound: OutboundSender,
    windowed: bool,
    /// Readable font (Optima on macOS, fallbacks on other platforms)
    font_data: Option<FontData>,
    /// Monospace font (Monaco on macOS, fallbacks on other platforms)
    #[allow(dead_code)]
    mono_font_data: Option<FontData>,

    // --- Voice input state ---
    voice_config: VoiceConfig,
    /// Current modifier state (tracks Shift held).
    modifiers: Modifiers,
    /// Live recording session; `Some` while Shift+Enter is held.
    recording: Option<voice::RecordingState>,
    /// UI feedback state for the dashboard renderer.
    voice_ui: VoiceUiState,
    /// Tokio runtime handle for spawning async transcription tasks.
    rt_handle: tokio::runtime::Handle,
    /// Channel for receiving transcription results from the async task.
    transcription_rx: std::sync::mpsc::Receiver<Result<voice::TranscriptionResult, String>>,
    transcription_tx: std::sync::mpsc::SyncSender<Result<voice::TranscriptionResult, String>>,

    // --- Setup mode state ---
    /// Current application mode.
    app_mode: AppMode,
    /// Characters typed in the setup URL field.
    setup_input: String,
}

impl App {
    /// Begin audio capture. Called on Shift+Enter keydown.
    fn start_recording(&mut self) {
        if !self.voice_config.enabled {
            return;
        }
        if self.recording.is_some() {
            return; // already recording
        }
        match voice::start_recording() {
            Ok(rec) => {
                self.recording = Some(rec);
                self.voice_ui = VoiceUiState::Recording;
                println!("[voice] Recording started");
            }
            Err(e) => {
                eprintln!("[voice] Failed to start recording: {}", e);
                self.voice_ui = VoiceUiState::Error(e.to_string());
            }
        }
    }

    /// Stop audio capture and spawn async transcription. Called on key release.
    fn stop_and_transcribe(&mut self) {
        let rec = match self.recording.take() {
            Some(r) => r,
            None => return,
        };

        self.voice_ui = VoiceUiState::Transcribing;
        println!("[voice] Recording stopped, transcribing...");

        let samples = voice::stop_and_collect(&rec);
        let sample_rate = rec.sample_rate;
        let channels = rec.channels;
        let config = self.voice_config.clone();
        let tx = self.transcription_tx.clone();

        self.rt_handle.spawn(async move {
            let result: Result<voice::TranscriptionResult, String> = async {
                let wav = voice::encode_to_wav(&samples, sample_rate, channels)
                    .map_err(|e| e.to_string())?;
                voice::transcribe(wav, &config)
                    .await
                    .map_err(|e| e.to_string())
            }
            .await;
            let _ = tx.send(result);
        });
    }

    /// Poll the transcription channel and handle any completed result.
    ///
    /// Called every frame from `window_event` RedrawRequested.
    fn poll_transcription(&mut self) {
        match self.transcription_rx.try_recv() {
            Ok(Ok(result)) => {
                let text = result.text.clone();
                println!("[voice] Transcribed: {:?}", text);

                if !text.is_empty() {
                    let msg = protocol::VoiceInputMessage::new(text.clone());
                    self.outbound.broadcast(msg.to_json());
                }

                self.voice_ui = VoiceUiState::Done(text);
            }
            Ok(Err(e)) => {
                eprintln!("[voice] Transcription failed: {}", e);
                self.voice_ui = VoiceUiState::Error(e);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
        }
    }

}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.state else {
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

        self.state = RenderState::Active {
            surface: Box::new(surface),
            valid_surface: true,
            window,
        };
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let RenderState::Active { window, .. } = &self.state {
            self.state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        // Verify this event is for our window
        let is_our_window = match &self.state {
            RenderState::Active { window, .. } => window.id() == window_id,
            _ => false,
        };
        if !is_our_window {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            // Track modifier state for push-to-talk detection.
            WindowEvent::ModifiersChanged(new_mods) => {
                let shift_was_held = self.modifiers.state().shift_key();
                self.modifiers = new_mods;
                let shift_now_held = self.modifiers.state().shift_key();

                // If Shift was released while recording, stop and transcribe.
                if shift_was_held && !shift_now_held && self.recording.is_some() {
                    self.stop_and_transcribe();
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),

            // ----------------------------------------------------------------
            // Setup mode: capture keyboard input for the URL field.
            // ----------------------------------------------------------------

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Backspace),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if self.app_mode == AppMode::Setup => {
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
            } if self.app_mode == AppMode::Setup => {
                let url = self.setup_input.trim().to_string();
                if url.starts_with("ws://") || url.starts_with("wss://") {
                    match save_server_url(&url) {
                        Ok(()) => {
                            println!("[setup] Saved server URL: {}", url);
                            // Extract whisper host from the WebSocket URL host.
                            if let Ok(parsed) = url::Url::parse(&url) {
                                if let Some(h) = parsed.host_str() {
                                    self.voice_config.whisper_host = h.to_string();
                                    println!("[setup] whisper host set to: {}", h);
                                }
                            }
                            // Spawn a fresh WebSocket client for the saved URL.
                            let rt = tokio::runtime::Builder::new_multi_thread()
                                .enable_all()
                                .build()
                                .expect("Failed to create runtime for setup");
                            let (instances, outbound) =
                                ws_client::spawn_clients(&rt, vec![url.clone()]);
                            // Leak the runtime so the spawned tasks keep running.
                            std::mem::forget(rt);
                            self.instances = instances;
                            self.outbound = outbound;
                            self.app_mode = AppMode::Dashboard;
                        }
                        Err(e) => eprintln!("[setup] Failed to save server URL: {}", e),
                    }
                } else {
                    eprintln!("[setup] URL must start with ws:// or wss://");
                }
            }

            // Cmd+V (macOS) or Ctrl+V (Linux/Windows): paste from clipboard in Setup mode.
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Character(ref c),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if self.app_mode == AppMode::Setup
                && (c.as_str() == "v" || c.as_str() == "V")
                && (self.modifiers.state().super_key()
                    || self.modifiers.state().control_key()) =>
            {
                match arboard::Clipboard::new().and_then(|mut cb| cb.get_text()) {
                    Ok(text) => {
                        self.setup_input.push_str(&text);
                        println!("[setup] Pasted {} chars from clipboard", text.len());
                    }
                    Err(e) => eprintln!("[setup] Clipboard read failed: {}", e),
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
            } if self.app_mode == AppMode::Setup => {
                self.setup_input.push_str(c.as_str());
            }

            // ----------------------------------------------------------------
            // Dashboard mode: voice input and hotkeys.
            // ----------------------------------------------------------------

            // Shift+Enter: start recording on press, stop on release.
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Enter),
                        state,
                        repeat: false,
                        ..
                    },
                ..
            } if self.modifiers.state().shift_key() && self.app_mode == AppMode::Dashboard => {
                match state {
                    ElementState::Pressed => self.start_recording(),
                    ElementState::Released => {
                        if self.recording.is_some() {
                            self.stop_and_transcribe();
                        }
                    }
                }
            }

            // 'V' key: toggle voice input on/off
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Character(ref c),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if (c.as_str() == "v" || c.as_str() == "V") && self.app_mode == AppMode::Dashboard => {
                self.voice_config.enabled = !self.voice_config.enabled;
                let status = if self.voice_config.enabled { "enabled" } else { "disabled" };
                println!("[voice] Voice input {}", status);
                if !self.voice_config.enabled {
                    self.recording = None;
                    self.voice_ui = VoiceUiState::Idle;
                }
            }

            // Press 'R' to request a fresh snapshot from all servers
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Character(ref c),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if (c.as_str() == "r" || c.as_str() == "R") && self.app_mode == AppMode::Dashboard => {
                // Could send request_snapshot to all connections
                // For now, updates come automatically
            }

            WindowEvent::Resized(size) => {
                if let RenderState::Active {
                    surface,
                    valid_surface,
                    ..
                } = &mut self.state
                {
                    if size.width != 0 && size.height != 0 {
                        self.context.resize_surface(surface, size.width, size.height);
                        *valid_surface = true;
                    } else {
                        *valid_surface = false;
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                // Poll for completed transcription results before borrowing surface.
                self.poll_transcription();

                // Render: borrow only the fields we need, avoiding a full &mut self borrow
                // while the surface reference is live.
                let RenderState::Active {
                    surface,
                    valid_surface: true,
                    window,
                } = &mut self.state
                else {
                    return;
                };

                self.scene.reset();

                let width = surface.config.width as f64;
                let height = surface.config.height as f64;
                let elapsed = self.start_time.elapsed().as_secs_f64();

                if self.app_mode == AppMode::Setup {
                    dashboard::render_setup_screen(
                        &mut self.scene,
                        width,
                        height,
                        &self.setup_input,
                        self.font_data.as_ref(),
                    );
                } else {
                    dashboard::render_dashboard(
                        &mut self.scene,
                        width,
                        height,
                        &self.instances,
                        elapsed,
                        self.font_data.as_ref(),
                        &self.voice_ui,
                        self.voice_config.enabled,
                    );
                }

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

                window.request_redraw();
            }

            _ => {}
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Create the Tokio runtime for async WebSocket clients and transcription
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    let rt_handle = runtime.handle().clone();

    println!("bisque-computer v{}", env!("CARGO_PKG_VERSION"));

    // Determine startup mode.
    // If the user explicitly passed --endpoints, honour them and go straight to dashboard.
    let explicit_endpoints =
        args.endpoints.len() != 1 || args.endpoints[0] != "ws://localhost:9100";

    let (app_mode, endpoints) = if explicit_endpoints {
        let eps = args.endpoints.clone();
        (AppMode::Dashboard, eps)
    } else if let Some(saved_url) = read_saved_server_url() {
        println!("Using saved server URL: {}", saved_url);
        (AppMode::Dashboard, vec![saved_url])
    } else {
        println!("No server URL configured — starting setup screen.");
        // Use a placeholder; no real connection is attempted until setup completes.
        (AppMode::Setup, vec!["ws://setup-placeholder:9100".to_string()])
    };

    if app_mode == AppMode::Dashboard {
        println!("Connecting to {} endpoint(s):", endpoints.len());
        for ep in &endpoints {
            println!("  - {}", ep);
        }
    }

    // Voice config — derive whisper host before endpoints is consumed by spawn_clients.
    let mut voice_config = VoiceConfig::default();
    if args.no_voice {
        voice_config.enabled = false;
    }
    // The same server that hosts the Lobster dashboard also runs whisper.cpp,
    // so extract the host from the WebSocket URL and route whisper calls there.
    if app_mode == AppMode::Dashboard && !endpoints.is_empty() {
        if let Ok(parsed) = url::Url::parse(&endpoints[0]) {
            if let Some(h) = parsed.host_str() {
                voice_config.whisper_host = h.to_string();
            }
        }
    }

    // Spawn WebSocket client tasks (consumes `endpoints`)
    let (instances, outbound) = ws_client::spawn_clients(&runtime, endpoints);

    // Load fonts at startup
    let font_data = dashboard::load_readable_font();
    let mono_font_data = dashboard::load_mono_font();

    if font_data.is_some() {
        println!("Loaded readable font (Optima/Helvetica/DejaVu Sans)");
    } else {
        eprintln!("Note: No system font found (Optima/Helvetica/DejaVu Sans).");
        eprintln!("All text will use bitmap font fallback.");
    }
    if mono_font_data.is_some() {
        println!("Loaded monospace font (Monaco/Menlo/DejaVu Sans Mono)");
    } else {
        eprintln!("Note: No monospace font found (Monaco/Menlo/DejaVu Sans Mono).");
    }

    if voice_config.enabled {
        println!("Voice input: enabled (Shift+Enter to record, V to toggle)");
        println!("  whisper host:  {}", voice_config.whisper_host);
        println!("  whisper port:  {}", voice_config.whisper_port);
    } else {
        println!("Voice input: disabled (--no-voice flag set)");
    }

    // Sync channel for transcription results (async task → main event loop)
    let (transcription_tx, transcription_rx) =
        std::sync::mpsc::sync_channel::<Result<voice::TranscriptionResult, String>>(4);

    let mut app = App {
        context: RenderContext::new(),
        renderers: vec![],
        state: RenderState::Suspended(None),
        scene: Scene::new(),
        start_time: Instant::now(),
        instances,
        outbound,
        windowed: args.windowed,
        font_data,
        mono_font_data,
        voice_config,
        modifiers: Modifiers::default(),
        recording: None,
        voice_ui: VoiceUiState::Idle,
        rt_handle,
        transcription_rx,
        transcription_tx,
        app_mode,
        setup_input: String::new(),
    };

    let event_loop = EventLoop::new()?;
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");

    // Clean shutdown of the Tokio runtime
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
