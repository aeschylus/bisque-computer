//! bisque-computer: Lobster Instance Dashboard
//!
//! A GPU-rendered fullscreen dashboard that connects to one or more Lobster
//! Dashboard WebSocket servers and visualizes system state in real time.
//!
//! Uses vello/wgpu for rendering and tokio-tungstenite for WebSocket communication.

mod dashboard;
#[allow(dead_code)]
mod protocol;
mod ws_client;

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use std::time::Instant;
use vello::peniko::color::palette;
use vello::peniko::FontData;
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window};

use vello::wgpu;

use ws_client::SharedInstances;

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
    windowed: bool,
    /// Readable font (Optima on macOS, fallbacks on other platforms)
    font_data: Option<FontData>,
    /// Monospace font (Monaco on macOS, fallbacks on other platforms)
    #[allow(dead_code)]
    mono_font_data: Option<FontData>,
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
        let (surface, valid_surface, window) = match &mut self.state {
            RenderState::Active {
                surface,
                valid_surface,
                window,
            } if window.id() == window_id => (surface, valid_surface, window.clone()),
            _ => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),

            // Press 'R' to request a fresh snapshot from all servers
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Character(ref c),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } if c.as_str() == "r" || c.as_str() == "R" => {
                // Could send request_snapshot to all connections
                // For now, updates come automatically
            }

            WindowEvent::Resized(size) => {
                if size.width != 0 && size.height != 0 {
                    self.context
                        .resize_surface(surface, size.width, size.height);
                    *valid_surface = true;
                } else {
                    *valid_surface = false;
                }
            }

            WindowEvent::RedrawRequested => {
                if !*valid_surface {
                    return;
                }

                self.scene.reset();

                let width = surface.config.width as f64;
                let height = surface.config.height as f64;
                let elapsed = self.start_time.elapsed().as_secs_f64();

                // Render the dashboard
                dashboard::render_dashboard(
                    &mut self.scene,
                    width,
                    height,
                    &self.instances,
                    elapsed,
                    self.font_data.as_ref(),
                );

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

                // Request another frame for continuous updates
                window.request_redraw();
            }

            _ => {}
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Create the Tokio runtime for async WebSocket clients
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    // Parse endpoint URLs
    let endpoints = if args.endpoints.len() == 1 && args.endpoints[0] == "ws://localhost:9100" {
        // Default: single local instance
        vec!["ws://localhost:9100".to_string()]
    } else {
        args.endpoints
    };

    println!("bisque-computer v{}", env!("CARGO_PKG_VERSION"));
    println!("Connecting to {} endpoint(s):", endpoints.len());
    for ep in &endpoints {
        println!("  - {}", ep);
    }

    // Spawn WebSocket client tasks
    let instances = ws_client::spawn_clients(&runtime, endpoints);

    // Load fonts at startup
    // Optima for readable text (all UI text), Monaco for monospace (code display)
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

    let mut app = App {
        context: RenderContext::new(),
        renderers: vec![],
        state: RenderState::Suspended(None),
        scene: Scene::new(),
        start_time: Instant::now(),
        instances,
        windowed: args.windowed,
        font_data,
        mono_font_data,
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
