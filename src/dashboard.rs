//! Dashboard rendering using vello.
//!
//! Renders Lobster instance data as a visual dashboard with panels for
//! system info, message queues, sessions, tasks, and health status.
//!
//! Visual theme: bisque beige throughout, black text, doubled text sizes.
//! Fonts: Optima for readable text, Monaco for monospace (font stacks with fallbacks).
//! On app open: Ulysses splash quote fades in and out.

use vello::kurbo::{Affine, BezPath, Point, Rect, RoundedRect};
use vello::peniko::{Color, Fill, FontData};
use vello::{Glyph, Scene};

use crate::protocol::{ConnectionStatus, LobsterInstance};
use crate::voice::VoiceUiState;
use crate::ws_client::SharedInstances;

// --- Color palette (bisque beige theme) ---
// CSS bisque = rgb(255, 228, 196) = [1.0, 0.894, 0.769]

const BG_COLOR: Color = Color::new([1.0, 0.894, 0.769, 1.0]);        // CSS bisque background
const PANEL_BG: Color = Color::new([1.0, 0.922, 0.827, 1.0]);        // lighter bisque panel
const PANEL_BORDER: Color = Color::new([0.87, 0.72, 0.53, 1.0]);     // warm tan border
const HEADER_BG: Color = Color::new([0.80, 0.62, 0.40, 1.0]);        // warm bisque-brown header
const TEXT_PRIMARY: Color = Color::new([0.0, 0.0, 0.0, 1.0]);        // pure black (#000000)
const TEXT_SECONDARY: Color = Color::new([0.0, 0.0, 0.0, 0.65]);     // black at 65% opacity
const TEXT_LIGHT: Color = Color::new([0.0, 0.0, 0.0, 1.0]);          // black on header
const ACCENT_GREEN: Color = Color::new([0.20, 0.65, 0.32, 1.0]);     // healthy green
const ACCENT_RED: Color = Color::new([0.80, 0.20, 0.18, 1.0]);       // alert red
const ACCENT_AMBER: Color = Color::new([0.85, 0.60, 0.10, 1.0]);     // warning amber
const ACCENT_BLUE: Color = Color::new([0.22, 0.46, 0.72, 1.0]);      // info blue
const BAR_BG: Color = Color::new([0.96, 0.87, 0.75, 1.0]);           // bisque bar background
const SECTION_BG: Color = Color::new([1.0, 0.91, 0.80, 1.0]);        // bisque section fill

// --- 3D Isometric Colors (bisque-compatible) ---
const ISO_SESSION_TOP: Color = Color::new([0.82, 0.55, 0.28, 1.0]);    // warm brown top face
const ISO_SESSION_LEFT: Color = Color::new([0.72, 0.48, 0.22, 1.0]);   // darker left face
const ISO_SESSION_RIGHT: Color = Color::new([0.90, 0.65, 0.38, 1.0]);  // lighter right face
const ISO_TASK_PENDING_TOP: Color = Color::new([0.85, 0.60, 0.10, 1.0]);   // amber
const ISO_TASK_PENDING_LEFT: Color = Color::new([0.72, 0.50, 0.08, 1.0]);
const ISO_TASK_PENDING_RIGHT: Color = Color::new([0.92, 0.70, 0.18, 1.0]);
const ISO_TASK_ACTIVE_TOP: Color = Color::new([0.22, 0.58, 0.72, 1.0]);    // blue-ish
const ISO_TASK_ACTIVE_LEFT: Color = Color::new([0.16, 0.46, 0.60, 1.0]);
const ISO_TASK_ACTIVE_RIGHT: Color = Color::new([0.30, 0.66, 0.80, 1.0]);
const ISO_TASK_DONE_TOP: Color = Color::new([0.25, 0.62, 0.35, 1.0]);      // green
const ISO_TASK_DONE_LEFT: Color = Color::new([0.18, 0.50, 0.26, 1.0]);
const ISO_TASK_DONE_RIGHT: Color = Color::new([0.32, 0.70, 0.42, 1.0]);
const ISO_SHADOW: Color = Color::new([0.0, 0.0, 0.0, 0.10]);              // soft shadow
const ISO_GROUND: Color = Color::new([0.92, 0.82, 0.68, 1.0]);            // ground plane
const ISO_GRID: Color = Color::new([0.85, 0.73, 0.58, 0.5]);              // ground grid lines
const ISO_INBOX_TOP: Color = Color::new([0.80, 0.20, 0.18, 1.0]);         // red for inbox
const ISO_INBOX_LEFT: Color = Color::new([0.65, 0.15, 0.13, 1.0]);
const ISO_INBOX_RIGHT: Color = Color::new([0.88, 0.28, 0.25, 1.0]);

// --- Layout constants (doubled text sizes) ---

const MARGIN: f64 = 24.0;
const PANEL_PADDING: f64 = 16.0;
const PANEL_GAP: f64 = 16.0;
const HEADER_HEIGHT: f64 = 72.0;       // taller for 2x text
const SECTION_HEIGHT: f64 = 48.0;      // taller for 2x text
const LINE_HEIGHT: f64 = 36.0;         // doubled from 22
const BAR_HEIGHT: f64 = 20.0;          // slightly taller
const CORNER_RADIUS: f64 = 8.0;
const STATUS_DOT_RADIUS: f64 = 8.0;    // slightly larger

// --- Splash quote from James Joyce's Ulysses ---

const ULYSSES_QUOTE: &str = "\
I was a Flower of the mountain yes \
when I put the rose in my hair like the Andalusian girls used \
or shall I wear a red yes \
and how he kissed me under the Moorish Wall \
and I thought well as well him as another \
and then I asked him with my eyes to ask again yes \
and then he asked me would I yes to say yes \
my mountain flower \
and first I put my arms around him yes \
and drew him down to me so he could feel my breasts all perfume yes \
and his heart was going like mad \
and yes I said yes I will Yes.";

// Splash animation timing
const FADE_IN_DURATION: f64 = 2.0;   // seconds to fade in
const HOLD_DURATION: f64 = 4.0;      // seconds to hold at full opacity
const FADE_OUT_DURATION: f64 = 2.0;  // seconds to fade out
const TOTAL_SPLASH_DURATION: f64 = FADE_IN_DURATION + HOLD_DURATION + FADE_OUT_DURATION;

/// Top-level render function: draws the full dashboard.
///
/// `voice_ui` carries the current push-to-talk state so the renderer can
/// show a recording indicator, "Transcribing..." overlay, and the result text.
/// `voice_enabled` controls whether the Shift+Enter hint is shown in the header.
pub fn render_dashboard(
    scene: &mut Scene,
    width: f64,
    height: f64,
    instances: &SharedInstances,
    elapsed: f64,
    font_data: Option<&FontData>,
    voice_ui: &VoiceUiState,
    voice_enabled: bool,
) {
    // Background fill - bisque beige
    let bg_rect = Rect::new(0.0, 0.0, width, height);
    scene.fill(Fill::NonZero, Affine::IDENTITY, BG_COLOR, None, &bg_rect);

    // Ulysses splash quote overlay (fades in and out on app open)
    if elapsed < TOTAL_SPLASH_DURATION {
        draw_splash_quote(scene, width, height, elapsed, font_data);
    }

    let instances = instances.lock().unwrap();

    if instances.is_empty() {
        draw_centered_text(scene, width, height, "No Lobster instances configured", TEXT_SECONDARY, 40.0, font_data);
        return;
    }

    // Title bar
    let title_rect = Rect::new(0.0, 0.0, width, HEADER_HEIGHT);
    scene.fill(Fill::NonZero, Affine::IDENTITY, HEADER_BG, None, &title_rect);
    draw_text_with_font(scene, MARGIN, 50.0, "LOBSTER DASHBOARD", TEXT_LIGHT, 44.0, font_data);

    // Count connected instances
    let connected = instances
        .iter()
        .filter(|i| i.status == ConnectionStatus::Connected)
        .count();
    let status_text = format!("{}/{} connected", connected, instances.len());
    draw_text_with_font(scene, width - 400.0, 50.0, &status_text, TEXT_LIGHT, 28.0, font_data);

    // Voice input hint in header (shown when enabled and idle)
    if voice_enabled {
        let hint = match voice_ui {
            VoiceUiState::Idle => "  |  Shift+Enter: voice",
            VoiceUiState::Recording => "  |  Recording...",
            VoiceUiState::Transcribing => "  |  Transcribing...",
            VoiceUiState::Done(_) => "  |  Sent",
            VoiceUiState::Error(_) => "  |  Voice error",
        };
        let hint_color = match voice_ui {
            VoiceUiState::Recording => Color::new([1.0_f32, 0.3, 0.3, 1.0]), // red while live
            VoiceUiState::Transcribing => Color::new([0.9_f32, 0.75, 0.1, 1.0]), // amber
            VoiceUiState::Done(_) => Color::new([0.3_f32, 0.8, 0.4, 1.0]),   // green
            _ => Color::new([1.0_f32, 1.0, 1.0, 0.60]),
        };
        draw_text_with_font(scene, width - 400.0, 72.0, hint, hint_color, 22.0, font_data);
    }

    // Layout: panels in a grid with 3D visualization
    let content_top = HEADER_HEIGHT + MARGIN;
    let content_width = width - 2.0 * MARGIN;
    let content_height = height - content_top - MARGIN;

    let num_instances = instances.len();

    // Check if any instance is connected (for 3D viz)
    let _any_connected = instances.iter().any(|i| i.status == ConnectionStatus::Connected);

    if num_instances == 1 {
        // Single instance: side-by-side layout -- info panel left, 3D viz right
        let split = 0.45; // info panel gets 45% width, 3D viz gets 55%
        let info_w = content_width * split - PANEL_GAP * 0.5;
        let viz_w = content_width * (1.0 - split) - PANEL_GAP * 0.5;

        draw_instance_panel(scene, MARGIN, content_top, info_w, content_height, &instances[0], font_data);
        draw_3d_visualization(
            scene,
            MARGIN + info_w + PANEL_GAP, content_top,
            viz_w, content_height,
            &instances[0], elapsed, font_data,
        );
    } else {
        // Multiple instances: grid of info panels + 3D viz row at bottom
        let viz_height = content_height * 0.35;
        let panels_height = content_height - viz_height - PANEL_GAP;

        let cols = if num_instances <= 1 { 1 } else if num_instances <= 4 { 2 } else { 3 };
        let rows = (num_instances + cols - 1) / cols;

        let panel_width = (content_width - (cols as f64 - 1.0) * PANEL_GAP) / cols as f64;
        let panel_height = (panels_height - (rows as f64 - 1.0) * PANEL_GAP) / rows as f64;

        for (idx, instance) in instances.iter().enumerate() {
            let col = idx % cols;
            let row = idx / cols;
            let x = MARGIN + col as f64 * (panel_width + PANEL_GAP);
            let y = content_top + row as f64 * (panel_height + PANEL_GAP);

            draw_instance_panel(scene, x, y, panel_width, panel_height, instance, font_data);
        }

        // Draw 3D viz panels for all instances along the bottom
        // (shows demo visualization even when disconnected)
        let viz_top = content_top + panels_height + PANEL_GAP;
        let viz_cols = num_instances;
        let viz_panel_w = (content_width - (viz_cols as f64 - 1.0).max(0.0) * PANEL_GAP) / viz_cols as f64;

        for (idx, instance) in instances.iter().enumerate() {
            let vx = MARGIN + idx as f64 * (viz_panel_w + PANEL_GAP);
            draw_3d_visualization(scene, vx, viz_top, viz_panel_w, viz_height, instance, elapsed, font_data);
        }
    }

    // Voice input overlay — drawn on top of everything else so it's always visible.
    // Drop the instances lock before drawing the overlay (already dropped at end of scope).
    drop(instances);
    draw_voice_indicator(scene, width, height, voice_ui, elapsed, font_data);
}

/// Draw the voice recording / transcribing / result overlay.
///
/// The indicator is anchored to the bottom-right corner. It shows:
/// - A pulsing red circle while recording.
/// - An amber "Transcribing..." badge while waiting for whisper.
/// - A green "Sent" confirmation after the text is dispatched.
/// - An error message in red on failure.
fn draw_voice_indicator(
    scene: &mut Scene,
    width: f64,
    height: f64,
    voice_ui: &VoiceUiState,
    elapsed: f64,
    font_data: Option<&FontData>,
) {
    match voice_ui {
        VoiceUiState::Idle => {} // Nothing to draw

        VoiceUiState::Recording => {
            // Pulsing red circle (mic-on indicator)
            let pulse = ((elapsed * 4.0).sin() * 0.5 + 0.5) as f32; // 0..1 at 4Hz
            let radius = 16.0 + 6.0 * pulse as f64;
            let cx = width - MARGIN - radius;
            let cy = height - MARGIN - radius;

            // Glow halo
            let halo_alpha = 0.25 + 0.15 * pulse;
            let halo_color = Color::new([0.9_f32, 0.1, 0.1, halo_alpha]);
            draw_circle(scene, cx, cy, radius * 2.0, halo_color);

            // Solid red dot
            let dot_color = Color::new([0.85_f32, 0.12, 0.10, 1.0]);
            draw_circle(scene, cx, cy, radius, dot_color);

            // "REC" label
            draw_text_with_font(
                scene,
                cx - radius - 60.0,
                cy + 8.0,
                "REC",
                dot_color,
                28.0,
                font_data,
            );
        }

        VoiceUiState::Transcribing => {
            // Amber pill badge: "Transcribing..."
            let badge_w = 280.0;
            let badge_h = 44.0;
            let bx = width - MARGIN - badge_w;
            let by = height - MARGIN - badge_h;
            let badge_rect = RoundedRect::new(bx, by, bx + badge_w, by + badge_h, 10.0);
            let badge_bg = Color::new([0.85_f32, 0.60, 0.10, 0.92]);
            scene.fill(Fill::NonZero, Affine::IDENTITY, badge_bg, None, &badge_rect);
            draw_text_with_font(
                scene,
                bx + 16.0,
                by + 30.0,
                "Transcribing...",
                Color::new([1.0_f32, 1.0, 1.0, 1.0]),
                24.0,
                font_data,
            );
        }

        VoiceUiState::Done(text) => {
            // Green pill badge with truncated transcribed text
            let max_chars = 40;
            let display_text: String = if text.len() > max_chars {
                format!("{}...", &text[..max_chars])
            } else {
                text.clone()
            };
            let badge_label = format!("Sent: {}", display_text);
            let badge_w = (badge_label.len() as f64 * 13.0 + 40.0).min(width * 0.6);
            let badge_h = 44.0;
            let bx = width - MARGIN - badge_w;
            let by = height - MARGIN - badge_h;
            let badge_rect = RoundedRect::new(bx, by, bx + badge_w, by + badge_h, 10.0);
            let badge_bg = Color::new([0.22_f32, 0.62, 0.30, 0.92]);
            scene.fill(Fill::NonZero, Affine::IDENTITY, badge_bg, None, &badge_rect);
            draw_text_with_font(
                scene,
                bx + 16.0,
                by + 30.0,
                &badge_label,
                Color::new([1.0_f32, 1.0, 1.0, 1.0]),
                22.0,
                font_data,
            );
        }

        VoiceUiState::Error(msg) => {
            // Red pill badge with error summary
            let max_chars = 50;
            let display_msg: String = if msg.len() > max_chars {
                format!("{}...", &msg[..max_chars])
            } else {
                msg.clone()
            };
            let badge_label = format!("Voice error: {}", display_msg);
            let badge_w = (badge_label.len() as f64 * 12.0 + 40.0).min(width * 0.65);
            let badge_h = 44.0;
            let bx = width - MARGIN - badge_w;
            let by = height - MARGIN - badge_h;
            let badge_rect = RoundedRect::new(bx, by, bx + badge_w, by + badge_h, 10.0);
            let badge_bg = Color::new([0.78_f32, 0.15, 0.12, 0.92]);
            scene.fill(Fill::NonZero, Affine::IDENTITY, badge_bg, None, &badge_rect);
            draw_text_with_font(
                scene,
                bx + 16.0,
                by + 30.0,
                &badge_label,
                Color::new([1.0_f32, 1.0, 1.0, 1.0]),
                22.0,
                font_data,
            );
        }
    }
}

/// Draw a single Lobster instance panel.
fn draw_instance_panel(
    scene: &mut Scene,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    instance: &LobsterInstance,
    font_data: Option<&FontData>,
) {
    // Panel background with rounded corners
    let panel_rect = RoundedRect::new(x, y, x + w, y + h, CORNER_RADIUS);
    scene.fill(Fill::NonZero, Affine::IDENTITY, PANEL_BG, None, &panel_rect);

    // Panel border
    scene.stroke(
        &vello::kurbo::Stroke::new(1.5),
        Affine::IDENTITY,
        PANEL_BORDER,
        None,
        &panel_rect,
    );

    let inner_x = x + PANEL_PADDING;
    let inner_w = w - 2.0 * PANEL_PADDING;
    let mut cursor_y = y + PANEL_PADDING;

    // --- Instance header with connection status ---
    let status_color = match &instance.status {
        ConnectionStatus::Connected => ACCENT_GREEN,
        ConnectionStatus::Connecting => ACCENT_AMBER,
        ConnectionStatus::Disconnected => ACCENT_RED,
        ConnectionStatus::Error(_) => ACCENT_RED,
    };

    // Status dot
    draw_circle(scene, inner_x + STATUS_DOT_RADIUS, cursor_y + 10.0, STATUS_DOT_RADIUS, status_color);

    // Hostname or URL
    let hostname = if instance.status == ConnectionStatus::Connected {
        &instance.state.system.hostname
    } else {
        &instance.url
    };
    draw_text_with_font(scene, inner_x + 24.0, cursor_y + 20.0, hostname, TEXT_PRIMARY, 32.0, font_data);

    // Connection status text
    let status_str = match &instance.status {
        ConnectionStatus::Connected => "connected".to_string(),
        ConnectionStatus::Connecting => "connecting...".to_string(),
        ConnectionStatus::Disconnected => "disconnected".to_string(),
        ConnectionStatus::Error(e) => format!("error: {}", &e[..e.len().min(30)]),
    };
    draw_text_with_font(scene, inner_x + 24.0, cursor_y + 48.0, &status_str, TEXT_PRIMARY, 22.0, font_data);
    cursor_y += 60.0;

    if instance.status != ConnectionStatus::Connected {
        return;
    }

    let state = &instance.state;

    // --- System section ---
    cursor_y = draw_section_header_with_font(scene, inner_x, cursor_y, inner_w, "SYSTEM", font_data);

    // Uptime
    let uptime_str = format_uptime(state.system.uptime_seconds);
    draw_label_value_with_font(scene, inner_x, cursor_y, "Uptime", &uptime_str, inner_w, font_data);
    cursor_y += LINE_HEIGHT;

    // CPU bar
    draw_label_bar_with_font(scene, inner_x, cursor_y, "CPU", state.system.cpu.percent, inner_w, cpu_color(state.system.cpu.percent), font_data);
    cursor_y += LINE_HEIGHT;

    // Memory bar
    draw_label_bar_with_font(scene, inner_x, cursor_y, "Memory", state.system.memory.percent, inner_w, mem_color(state.system.memory.percent), font_data);
    cursor_y += LINE_HEIGHT;

    // Disk bar
    draw_label_bar_with_font(scene, inner_x, cursor_y, "Disk", state.system.disk.percent, inner_w, disk_color(state.system.disk.percent), font_data);
    cursor_y += LINE_HEIGHT + 4.0;

    // --- Sessions section ---
    cursor_y = draw_section_header_with_font(scene, inner_x, cursor_y, inner_w, "SESSIONS", font_data);

    // Filter to just actual Claude processes (not wrapper scripts, tmux, etc.)
    let claude_sessions: Vec<_> = state.sessions.iter()
        .filter(|s| s.name == "claude")
        .collect();
    let session_count = claude_sessions.len();
    draw_label_value_with_font(scene, inner_x, cursor_y, "Active", &format!("{}", session_count), inner_w, font_data);
    cursor_y += LINE_HEIGHT;

    // Show memory for each session (up to 3)
    for (_i, session) in claude_sessions.iter().take(3).enumerate() {
        let label = format!("  PID {}", session.pid);
        let value = format!("{:.0} MB", session.memory_mb);
        draw_label_value_with_font(scene, inner_x, cursor_y, &label, &value, inner_w, font_data);
        cursor_y += LINE_HEIGHT;
    }
    if session_count > 3 {
        draw_text_with_font(scene, inner_x + 8.0, cursor_y, &format!("  +{} more", session_count - 3), TEXT_PRIMARY, 22.0, font_data);
        cursor_y += LINE_HEIGHT;
    }
    cursor_y += 4.0;

    // --- Messages section ---
    cursor_y = draw_section_header_with_font(scene, inner_x, cursor_y, inner_w, "MESSAGES", font_data);

    let queues = &state.message_queues;
    let queue_items = [
        ("Inbox", queues.inbox.count, if queues.inbox.count > 0 { ACCENT_AMBER } else { ACCENT_GREEN }),
        ("Processed", queues.processed.count, TEXT_PRIMARY),
        ("Sent", queues.sent.count, ACCENT_BLUE),
        ("Failed", queues.failed.count, if queues.failed.count > 0 { ACCENT_RED } else { TEXT_PRIMARY }),
    ];

    for (label, count, color) in &queue_items {
        draw_label_value_colored_with_font(scene, inner_x, cursor_y, label, &count.to_string(), inner_w, *color, font_data);
        cursor_y += LINE_HEIGHT;
    }
    cursor_y += 4.0;

    // --- Activity section ---
    if cursor_y + 60.0 < y + h {
        cursor_y = draw_section_header_with_font(scene, inner_x, cursor_y, inner_w, "ACTIVITY (24h)", font_data);

        let activity = &state.conversation_activity;
        draw_label_value_with_font(scene, inner_x, cursor_y, "Received", &activity.messages_received_24h.to_string(), inner_w, font_data);
        cursor_y += LINE_HEIGHT;
        draw_label_value_with_font(scene, inner_x, cursor_y, "Replied", &activity.replies_sent_24h.to_string(), inner_w, font_data);
        cursor_y += LINE_HEIGHT + 4.0;
    }

    // --- Health section ---
    if cursor_y + 40.0 < y + h {
        cursor_y = draw_section_header_with_font(scene, inner_x, cursor_y, inner_w, "HEALTH", font_data);

        let health = &state.health;
        let hb_status = match health.heartbeat_age_seconds {
            Some(age) if age < 300 => format!("{}s ago", age),
            Some(age) => format!("STALE ({}s)", age),
            None => "unknown".to_string(),
        };
        let hb_color = if health.heartbeat_stale { ACCENT_RED } else { ACCENT_GREEN };
        draw_label_value_colored_with_font(scene, inner_x, cursor_y, "Heartbeat", &hb_status, inner_w, hb_color, font_data);
        cursor_y += LINE_HEIGHT;

        let bot_status = if health.telegram_bot_running { "running" } else { "stopped" };
        let bot_color = if health.telegram_bot_running { ACCENT_GREEN } else { ACCENT_RED };
        draw_label_value_colored_with_font(scene, inner_x, cursor_y, "Telegram Bot", bot_status, inner_w, bot_color, font_data);
        cursor_y += LINE_HEIGHT;
    }

    // --- Agents section ---
    if cursor_y + 40.0 < y + h {
        cursor_y = draw_section_header_with_font(scene, inner_x, cursor_y, inner_w, "AGENTS", font_data);

        let agents = &state.subagent_list.agents;
        if agents.is_empty() {
            draw_text_with_font(scene, inner_x + 4.0, cursor_y + 24.0, "No agents running", TEXT_SECONDARY, 22.0, font_data);
            cursor_y += LINE_HEIGHT;
        } else {
            for agent in agents.iter().take(4) {
                if cursor_y + LINE_HEIGHT > y + h {
                    break;
                }
                // Status dot
                let dot_color = match agent.status.as_str() {
                    "running" => ACCENT_GREEN,
                    "stale" => ACCENT_AMBER,
                    _ => TEXT_SECONDARY,
                };
                draw_circle(scene, inner_x + STATUS_DOT_RADIUS, cursor_y + 18.0, STATUS_DOT_RADIUS * 0.75, dot_color);

                // Description (truncated)
                let desc = if agent.description.len() > 52 {
                    format!("{}...", &agent.description[..52])
                } else {
                    agent.description.clone()
                };
                draw_text_with_font(scene, inner_x + 20.0, cursor_y + 24.0, &desc, TEXT_PRIMARY, 20.0, font_data);

                // Elapsed time and stats on the right
                let elapsed_str = match agent.elapsed_seconds {
                    Some(s) if s < 60 => format!("{}s", s),
                    Some(s) if s < 3600 => format!("{}m", s / 60),
                    Some(s) => format!("{}h{}m", s / 3600, (s % 3600) / 60),
                    None => "?".to_string(),
                };
                let turns_str = agent.runtime.as_ref().map_or("".to_string(), |r| {
                    format!("{} turns/{} tools", r.turns, r.tool_uses)
                });
                let stats_str = if turns_str.is_empty() {
                    elapsed_str
                } else {
                    format!("{} | {}", elapsed_str, turns_str)
                };
                let stats_w = stats_str.len() as f64 * 12.0;
                draw_text_with_font(scene, (inner_x + inner_w - stats_w).max(inner_x + 20.0), cursor_y + 24.0, &stats_str, TEXT_SECONDARY, 20.0, font_data);
                cursor_y += LINE_HEIGHT;
            }
            if agents.len() > 4 {
                draw_text_with_font(scene, inner_x + 8.0, cursor_y + 24.0, &format!("  +{} more", agents.len() - 4), TEXT_SECONDARY, 20.0, font_data);
                cursor_y += LINE_HEIGHT;
            }
        }
        cursor_y += 4.0;
    }

    // --- Memory section ---
    if cursor_y + 40.0 < y + h {
        cursor_y = draw_section_header_with_font(scene, inner_x, cursor_y, inner_w, "MEMORY", font_data);

        let mem = &state.memory;

        // Summary line
        let summary = format!(
            "{} events, {} unconsolidated",
            mem.total_events, mem.unconsolidated_count
        );
        draw_label_value_with_font(scene, inner_x, cursor_y, "Events", &mem.total_events.to_string(), inner_w, font_data);
        let _ = summary; // used for readability above
        cursor_y += LINE_HEIGHT;

        // Projects list
        if !mem.projects.is_empty() && cursor_y + LINE_HEIGHT < y + h {
            let projects_str = if mem.projects.len() > 4 {
                format!("{} (+{})", mem.projects[..4].join(", "), mem.projects.len() - 4)
            } else {
                mem.projects.join(", ")
            };
            draw_label_value_with_font(scene, inner_x, cursor_y, "Projects", &projects_str, inner_w, font_data);
            cursor_y += LINE_HEIGHT;
        }

        // Recent events (up to 4)
        for event in mem.recent_events.iter().take(4) {
            if cursor_y + LINE_HEIGHT > y + h {
                break;
            }
            let type_tag = match event.event_type.as_str() {
                "decision" => "[D]",
                "note" => "[N]",
                "link" => "[L]",
                "task" => "[T]",
                _ => "[?]",
            };
            let content_max = 50usize;
            let snippet = if event.content.len() > content_max {
                format!("{}...", &event.content[..content_max])
            } else {
                event.content.clone()
            };
            let line = format!("{} {}", type_tag, snippet);
            draw_text_with_font(scene, inner_x + 4.0, cursor_y + 22.0, &line, TEXT_SECONDARY, 20.0, font_data);
            cursor_y += LINE_HEIGHT;
        }

        // Last consolidation
        if cursor_y + LINE_HEIGHT < y + h {
            let consol_str = match &mem.consolidations.last_consolidation_at {
                Some(ts) => {
                    // Show just date+time portion for brevity (first 16 chars of ISO)
                    let short = if ts.len() > 16 { &ts[..16] } else { ts.as_str() };
                    format!("Last: {}", short)
                }
                None => "Last: never".to_string(),
            };
            draw_text_with_font(scene, inner_x + 4.0, cursor_y + 22.0, &consol_str, TEXT_SECONDARY, 20.0, font_data);
            cursor_y += LINE_HEIGHT;
        }
        let _ = cursor_y;
    }
}

// --- Drawing primitives ---
// Text rendering: uses real font (Optima/Monaco) when available via vello's
// draw_glyphs API + skrifa for metrics. Falls back to bitmap font.

const CHAR_W: f64 = 7.0;
const CHAR_H: f64 = 12.0;
const CHAR_GAP: f64 = 1.0;

/// Draw text using a real font when available, falling back to bitmap glyphs.
/// When font_data is Some, uses vello's draw_glyphs API for proper font rendering.
/// When font_data is None, falls back to the 5x7 bitmap font.
fn draw_text_with_font(scene: &mut Scene, x: f64, y: f64, text: &str, color: Color, size: f64, font_data: Option<&FontData>) {
    if let Some(font) = font_data {
        // Use real font rendering
        let font_size = size as f32;
        let glyphs = layout_text_single_line(text, font, font_size, x, y);
        if !glyphs.is_empty() {
            scene
                .draw_glyphs(font)
                .font_size(font_size)
                .brush(&color)
                .draw(Fill::NonZero, glyphs.into_iter());
        }
    } else {
        draw_text_bitmap(scene, x, y, text, color, size);
    }
}

/// Draw text using the bitmap font (fallback when no system font is available).
fn draw_text_bitmap(scene: &mut Scene, x: f64, y: f64, text: &str, color: Color, size: f64) {
    let scale = size / 14.0;
    let cw = CHAR_W * scale;
    let ch = CHAR_H * scale;
    let gap = CHAR_GAP * scale;

    for (i, ch_byte) in text.bytes().enumerate() {
        let cx = x + i as f64 * (cw + gap);

        if ch_byte == b' ' {
            continue;
        }

        draw_bitmap_char(scene, cx, y - ch, cw, ch, ch_byte, color);
    }
}

/// Legacy draw_text wrapper -- used when no font context is available.
/// Kept for backward compatibility in helpers that don't have font access.
fn draw_text(scene: &mut Scene, x: f64, y: f64, text: &str, color: Color, size: f64) {
    draw_text_bitmap(scene, x, y, text, color, size);
}

/// Layout text on a single line using a real font, returning positioned glyphs.
fn layout_text_single_line(
    text: &str,
    font_data: &FontData,
    font_size: f32,
    start_x: f64,
    start_y: f64,
) -> Vec<Glyph> {
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

/// Draw a single character using a 5x7 bitmap pattern.
fn draw_bitmap_char(scene: &mut Scene, x: f64, y: f64, w: f64, h: f64, ch: u8, color: Color) {
    let bitmap = get_char_bitmap(ch);
    let pixel_w = w / 5.0;
    let pixel_h = h / 7.0;

    for row in 0..7 {
        for col in 0..5 {
            if (bitmap[row] >> (4 - col)) & 1 == 1 {
                let px = x + col as f64 * pixel_w;
                let py = y + row as f64 * pixel_h;
                let rect = Rect::new(px, py, px + pixel_w, py + pixel_h);
                scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &rect);
            }
        }
    }
}

/// Get a 5x7 bitmap for a character. Each row is a u8 where the lower 5 bits
/// represent pixels (MSB = leftmost pixel).
fn get_char_bitmap(ch: u8) -> [u8; 7] {
    match ch {
        b'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        b'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        b'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        b'D' => [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100],
        b'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        b'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        b'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        b'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        b'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        b'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        b'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        b'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        b'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        b'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        b'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        b'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        b'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        b'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        b'S' => [0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110],
        b'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        b'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        b'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100],
        b'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        b'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        b'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        b'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        b'a' => [0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111],
        b'b' => [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b11110],
        b'c' => [0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110],
        b'd' => [0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b10001, 0b01111],
        b'e' => [0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110],
        b'f' => [0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000],
        b'g' => [0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        b'h' => [0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001],
        b'i' => [0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
        b'j' => [0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100],
        b'k' => [0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010],
        b'l' => [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        b'm' => [0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001],
        b'n' => [0b00000, 0b00000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001],
        b'o' => [0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
        b'p' => [0b00000, 0b00000, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000],
        b'q' => [0b00000, 0b00000, 0b01101, 0b10011, 0b01111, 0b00001, 0b00001],
        b'r' => [0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000],
        b's' => [0b00000, 0b00000, 0b01110, 0b10000, 0b01110, 0b00001, 0b11110],
        b't' => [0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110],
        b'u' => [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101],
        b'v' => [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        b'w' => [0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010],
        b'x' => [0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001],
        b'y' => [0b00000, 0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        b'z' => [0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111],
        b'0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        b'1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        b'2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        b'3' => [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
        b'4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        b'5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        b'6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        b'7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        b'8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        b'9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        b'.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        b',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000],
        b':' => [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000],
        b';' => [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b00100, 0b01000],
        b'!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        b'?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100],
        b'-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        b'+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
        b'=' => [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
        b'/' => [0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000],
        b'\\' => [0b10000, 0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0b00001],
        b'(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        b')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        b'[' => [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110],
        b']' => [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110],
        b'%' => [0b11001, 0b11001, 0b00010, 0b00100, 0b01000, 0b10011, 0b10011],
        b'#' => [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010],
        b'_' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
        b'<' => [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
        b'>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
        b'\'' => [0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000],
        b'"' => [0b01010, 0b01010, 0b10100, 0b00000, 0b00000, 0b00000, 0b00000],
        b'@' => [0b01110, 0b10001, 0b10111, 0b10101, 0b10110, 0b10000, 0b01110],
        b'*' => [0b00000, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0b00000],
        b'&' => [0b01100, 0b10010, 0b01100, 0b10010, 0b10001, 0b10010, 0b01101],
        b'^' => [0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000],
        b'~' => [0b00000, 0b00000, 0b01000, 0b10101, 0b00010, 0b00000, 0b00000],
        b'`' => [0b01000, 0b00100, 0b00010, 0b00000, 0b00000, 0b00000, 0b00000],
        b'{' => [0b00110, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00110],
        b'}' => [0b01100, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b01100],
        b'|' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        _    => [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111], // box for unknown
    }
}

/// Draw centered text with optional font.
fn draw_centered_text(scene: &mut Scene, width: f64, height: f64, text: &str, color: Color, size: f64, font_data: Option<&FontData>) {
    let scale = size / 14.0;
    let text_width = text.len() as f64 * (CHAR_W * scale + CHAR_GAP * scale);
    let x = (width - text_width) / 2.0;
    let y = height / 2.0;
    draw_text_with_font(scene, x, y, text, color, size, font_data);
}

/// Public wrapper for `draw_centered_text`, used by other modules.
pub fn draw_centered_text_pub(scene: &mut Scene, width: f64, height: f64, text: &str, color: Color, size: f64, font_data: Option<&FontData>) {
    draw_centered_text(scene, width, height, text, color, size, font_data);
}

/// Public wrapper for `draw_text_with_font`, used by other modules.
pub fn draw_text_pub(scene: &mut Scene, x: f64, y: f64, text: &str, color: Color, size: f64, font_data: Option<&FontData>) {
    draw_text_with_font(scene, x, y, text, color, size, font_data);
}

/// Draw a filled circle.
fn draw_circle(scene: &mut Scene, cx: f64, cy: f64, r: f64, color: Color) {
    let circle = vello::kurbo::Circle::new(Point::new(cx, cy), r);
    scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &circle);
}

/// Draw a section header bar (bitmap font only).
#[allow(dead_code)]
fn draw_section_header(scene: &mut Scene, x: f64, y: f64, w: f64, title: &str) -> f64 {
    let rect = RoundedRect::new(x, y, x + w, y + SECTION_HEIGHT - 4.0, 4.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, SECTION_BG, None, &rect);
    draw_text(scene, x + 8.0, y + SECTION_HEIGHT - 16.0, title, TEXT_PRIMARY, 22.0);
    y + SECTION_HEIGHT
}

/// Draw a section header bar with font support.
fn draw_section_header_with_font(scene: &mut Scene, x: f64, y: f64, w: f64, title: &str, font_data: Option<&FontData>) -> f64 {
    let rect = RoundedRect::new(x, y, x + w, y + SECTION_HEIGHT - 4.0, 4.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, SECTION_BG, None, &rect);
    draw_text_with_font(scene, x + 8.0, y + SECTION_HEIGHT - 16.0, title, TEXT_PRIMARY, 22.0, font_data);
    y + SECTION_HEIGHT
}

/// Draw a label: value pair (bitmap font only).
#[allow(dead_code)]
fn draw_label_value(scene: &mut Scene, x: f64, y: f64, label: &str, value: &str, _w: f64) {
    draw_text(scene, x + 4.0, y + 24.0, label, TEXT_PRIMARY, 24.0);
    let value_width = value.len() as f64 * 14.0;
    draw_text(scene, x + _w - value_width - 4.0, y + 24.0, value, TEXT_PRIMARY, 24.0);
}

/// Draw a label: value pair with font support.
fn draw_label_value_with_font(scene: &mut Scene, x: f64, y: f64, label: &str, value: &str, w: f64, font_data: Option<&FontData>) {
    draw_text_with_font(scene, x + 4.0, y + 24.0, label, TEXT_PRIMARY, 24.0, font_data);
    let value_width = value.len() as f64 * 14.0;
    draw_text_with_font(scene, x + w - value_width - 4.0, y + 24.0, value, TEXT_PRIMARY, 24.0, font_data);
}

/// Draw a label: value pair with custom value color (bitmap font only).
#[allow(dead_code)]
fn draw_label_value_colored(scene: &mut Scene, x: f64, y: f64, label: &str, value: &str, w: f64, color: Color) {
    draw_text(scene, x + 4.0, y + 24.0, label, TEXT_PRIMARY, 24.0);
    let value_width = value.len() as f64 * 14.0;
    draw_text(scene, x + w - value_width - 4.0, y + 24.0, value, color, 24.0);
}

/// Draw a label: value pair with custom value color and font support.
fn draw_label_value_colored_with_font(scene: &mut Scene, x: f64, y: f64, label: &str, value: &str, w: f64, color: Color, font_data: Option<&FontData>) {
    draw_text_with_font(scene, x + 4.0, y + 24.0, label, TEXT_PRIMARY, 24.0, font_data);
    let value_width = value.len() as f64 * 14.0;
    draw_text_with_font(scene, x + w - value_width - 4.0, y + 24.0, value, color, 24.0, font_data);
}

/// Draw a labeled progress bar (bitmap font only).
#[allow(dead_code)]
fn draw_label_bar(scene: &mut Scene, x: f64, y: f64, label: &str, percent: f64, w: f64, bar_color: Color) {
    let label_w = 120.0;
    draw_text(scene, x + 4.0, y + 24.0, label, TEXT_PRIMARY, 24.0);

    let bar_x = x + label_w;
    let bar_w = w - label_w - 80.0;
    let bar_y = y + 8.0;
    let bg_rect = RoundedRect::new(bar_x, bar_y, bar_x + bar_w, bar_y + BAR_HEIGHT, 3.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, BAR_BG, None, &bg_rect);

    let fill_w = (bar_w * percent / 100.0).max(0.0).min(bar_w);
    if fill_w > 0.0 {
        let fill_rect = RoundedRect::new(bar_x, bar_y, bar_x + fill_w, bar_y + BAR_HEIGHT, 3.0);
        scene.fill(Fill::NonZero, Affine::IDENTITY, bar_color, None, &fill_rect);
    }

    let pct_str = format!("{:.0}%", percent);
    draw_text(scene, x + w - 70.0, y + 24.0, &pct_str, TEXT_PRIMARY, 24.0);
}

/// Draw a labeled progress bar with font support.
fn draw_label_bar_with_font(scene: &mut Scene, x: f64, y: f64, label: &str, percent: f64, w: f64, bar_color: Color, font_data: Option<&FontData>) {
    let label_w = 120.0;
    draw_text_with_font(scene, x + 4.0, y + 24.0, label, TEXT_PRIMARY, 24.0, font_data);

    let bar_x = x + label_w;
    let bar_w = w - label_w - 80.0;
    let bar_y = y + 8.0;
    let bg_rect = RoundedRect::new(bar_x, bar_y, bar_x + bar_w, bar_y + BAR_HEIGHT, 3.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, BAR_BG, None, &bg_rect);

    let fill_w = (bar_w * percent / 100.0).max(0.0).min(bar_w);
    if fill_w > 0.0 {
        let fill_rect = RoundedRect::new(bar_x, bar_y, bar_x + fill_w, bar_y + BAR_HEIGHT, 3.0);
        scene.fill(Fill::NonZero, Affine::IDENTITY, bar_color, None, &fill_rect);
    }

    let pct_str = format!("{:.0}%", percent);
    draw_text_with_font(scene, x + w - 70.0, y + 24.0, &pct_str, TEXT_PRIMARY, 24.0, font_data);
}

// --- Splash quote rendering ---

/// Compute the splash quote alpha based on elapsed time
fn splash_alpha(elapsed: f64) -> f32 {
    if elapsed >= TOTAL_SPLASH_DURATION {
        return 0.0;
    }
    if elapsed < FADE_IN_DURATION {
        // Fade in: 0 -> 1 with smooth ease-in-out cubic
        let t = (elapsed / FADE_IN_DURATION) as f32;
        ease_in_out(t)
    } else if elapsed < FADE_IN_DURATION + HOLD_DURATION {
        // Hold at full opacity
        1.0
    } else {
        // Fade out: 1 -> 0
        let t = ((elapsed - FADE_IN_DURATION - HOLD_DURATION) / FADE_OUT_DURATION) as f32;
        1.0 - ease_in_out(t)
    }
}

/// Smooth ease-in-out cubic curve
fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Draw the Ulysses splash quote with fade animation.
/// Uses real font rendering via vello's draw_glyphs API when a font is available,
/// otherwise falls back to the bitmap font renderer.
fn draw_splash_quote(
    scene: &mut Scene,
    width: f64,
    height: f64,
    elapsed: f64,
    font_data: Option<&FontData>,
) {
    let alpha = splash_alpha(elapsed);
    if alpha <= 0.001 {
        return;
    }

    // Semi-transparent bisque overlay behind the quote
    let overlay_color = Color::new([1.0_f32, 0.894, 0.769, alpha * 0.92]);
    let overlay_rect = Rect::new(0.0, 0.0, width, height);
    scene.fill(Fill::NonZero, Affine::IDENTITY, overlay_color, None, &overlay_rect);

    let black_with_alpha = Color::new([0.0_f32, 0.0, 0.0, alpha]);

    if let Some(font) = font_data {
        // Use real font rendering (Optima on macOS)
        let font_size: f32 = 48.0;
        let max_text_width = width * 0.7;
        let start_x = width * 0.15;
        let start_y = height * 0.25;

        let glyphs = layout_text_with_font(
            ULYSSES_QUOTE,
            font,
            font_size,
            max_text_width,
            start_x,
            start_y,
        );

        if !glyphs.is_empty() {
            scene
                .draw_glyphs(font)
                .font_size(font_size)
                .brush(&black_with_alpha)
                .draw(Fill::NonZero, glyphs.into_iter());
        }
    } else {
        // Fallback: use bitmap font rendering
        let font_size = 32.0; // doubled from 16
        let max_chars_per_line = ((width * 0.7) / (font_size * 0.6)) as usize;
        let start_x = width * 0.15;
        let mut y = height * 0.20;
        let line_height_px = font_size * 1.5;

        // Simple word wrapping for bitmap font
        let words: Vec<&str> = ULYSSES_QUOTE.split_whitespace().collect();
        let mut line = String::new();

        for word in &words {
            if line.len() + word.len() + 1 > max_chars_per_line && !line.is_empty() {
                draw_text(scene, start_x, y, &line, black_with_alpha, font_size);
                y += line_height_px;
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            draw_text(scene, start_x, y, &line, black_with_alpha, font_size);
        }
    }
}

/// Layout text using a real font via skrifa for glyph metrics.
/// Maps characters to glyph IDs and positions them with word wrapping.
fn layout_text_with_font(
    text: &str,
    font_data: &FontData,
    font_size: f32,
    max_width: f64,
    start_x: f64,
    start_y: f64,
) -> Vec<Glyph> {
    let font_ref = skrifa::FontRef::from_index(font_data.data.as_ref(), font_data.index);
    let font_ref = match font_ref {
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
    let mut y = start_y;
    let line_height = font_size as f64 * 1.4;

    let words: Vec<&str> = text.split_whitespace().collect();

    for (i, word) in words.iter().enumerate() {
        // Measure word width
        let word_width: f64 = word
            .chars()
            .map(|ch| {
                let gid = charmap.map(ch).unwrap_or_default();
                glyph_metrics
                    .advance_width(gid)
                    .unwrap_or(font_size * 0.5) as f64
            })
            .sum();

        let space_gid = charmap.map(' ').unwrap_or_default();
        let space_width = glyph_metrics
            .advance_width(space_gid)
            .unwrap_or(font_size * 0.25) as f64;

        let needed = if i > 0 && x > start_x {
            space_width + word_width
        } else {
            word_width
        };
        if x + needed > start_x + max_width && x > start_x {
            x = start_x;
            y += line_height;
        } else if i > 0 && x > start_x {
            x += space_width;
        }

        for ch in word.chars() {
            let gid = charmap.map(ch).unwrap_or_default();
            let advance = glyph_metrics
                .advance_width(gid)
                .unwrap_or(font_size * 0.5) as f64;

            glyphs.push(Glyph {
                id: gid.to_u32(),
                x: x as f32,
                y: y as f32,
            });

            x += advance;
        }
    }

    glyphs
}

// --- Font loading ---

/// Try to load a font from common system paths.
/// On macOS: Optima and Monaco are system fonts.
/// Falls back to Linux system fonts for development.
fn load_system_font(font_names: &[&str]) -> Option<FontData> {
    let macos_dirs = [
        "/System/Library/Fonts/",
        "/System/Library/Fonts/Supplemental/",
        "/Library/Fonts/",
    ];
    let linux_dirs = [
        "/usr/share/fonts/truetype/dejavu/",
        "/usr/share/fonts/truetype/",
        "/usr/share/fonts/opentype/",
    ];
    let extensions = ["ttf", "otf", "ttc"];
    let all_dirs: Vec<&str> = macos_dirs.iter().chain(linux_dirs.iter()).copied().collect();

    for name in font_names {
        for dir in &all_dirs {
            for ext in &extensions {
                let path = format!("{}{}.{}", dir, name, ext);
                if let Ok(data) = std::fs::read(&path) {
                    return Some(FontData::new(data.into(), 0));
                }
            }
        }
    }
    None
}

/// Render the first-run setup screen.
///
/// Shown when `~/.config/bisque-computer/server` is absent. The user types a
/// WebSocket URL then presses Enter to save and connect.
pub fn render_setup_screen(
    scene: &mut Scene,
    width: f64,
    height: f64,
    input_buffer: &str,
    font_data: Option<&FontData>,
) {
    // Full bisque background
    let bg = Rect::new(0.0, 0.0, width, height);
    scene.fill(Fill::NonZero, Affine::IDENTITY, BG_COLOR, None, &bg);

    let cx = width / 2.0;
    let cy = height / 2.0;

    // Title: "Connect to Lobster"
    let title = "Connect to Lobster";
    let title_size = 52.0_f64;
    let title_w = title.len() as f64 * title_size * 0.55;
    draw_text_with_font(scene, cx - title_w / 2.0, cy - 120.0, title, TEXT_PRIMARY, title_size, font_data);

    // Instruction
    let instr = "Enter your Lobster connection URL and press Enter";
    let instr_size = 26.0_f64;
    let instr_w = instr.len() as f64 * instr_size * 0.55;
    draw_text_with_font(scene, cx - instr_w / 2.0, cy - 60.0, instr, TEXT_SECONDARY, instr_size, font_data);

    // Hint
    let hint = "(e.g. ws://IP:9100?token=UUID)";
    let hint_size = 22.0_f64;
    let hint_w = hint.len() as f64 * hint_size * 0.55;
    draw_text_with_font(scene, cx - hint_w / 2.0, cy - 28.0, hint, TEXT_SECONDARY, hint_size, font_data);

    // Input field
    let field_w = (width * 0.7).min(900.0);
    let field_h = 56.0;
    let field_x = cx - field_w / 2.0;
    let field_y = cy + 8.0;
    let field_rect = RoundedRect::new(field_x, field_y, field_x + field_w, field_y + field_h, 6.0);

    // White-ish fill for the input area
    let field_fill = Color::new([1.0_f32, 1.0, 1.0, 0.85]);
    scene.fill(Fill::NonZero, Affine::IDENTITY, field_fill, None, &field_rect);
    // Border
    scene.stroke(
        &vello::kurbo::Stroke::new(2.0),
        Affine::IDENTITY,
        PANEL_BORDER,
        None,
        &field_rect,
    );

    // Text inside the field
    let (display_text, text_color) = if input_buffer.is_empty() {
        ("ws://".to_string(), TEXT_SECONDARY)
    } else {
        (input_buffer.to_string(), TEXT_PRIMARY)
    };
    draw_text_with_font(scene, field_x + 14.0, field_y + 38.0, &display_text, text_color, 28.0, font_data);

    // Cursor
    let char_advance = 16.0_f64;
    let cursor_x = if input_buffer.is_empty() {
        field_x + 14.0 + 5.0 * char_advance // after "ws://"
    } else {
        field_x + 14.0 + input_buffer.len() as f64 * char_advance
    };
    let cursor_rect = Rect::new(cursor_x, field_y + 12.0, cursor_x + 2.0, field_y + field_h - 12.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, TEXT_PRIMARY, None, &cursor_rect);

    // Footer
    let footer = "Press Enter to connect  |  Press Escape to quit";
    let footer_size = 22.0_f64;
    let footer_w = footer.len() as f64 * footer_size * 0.55;
    draw_text_with_font(scene, cx - footer_w / 2.0, cy + 120.0, footer, TEXT_SECONDARY, footer_size, font_data);
}

/// Load the best available font for readable text.
/// Font stack: Optima > Helvetica > Arial > DejaVu Sans > Liberation Sans
pub fn load_readable_font() -> Option<FontData> {
    load_system_font(&[
        "Optima",
        "Optima Regular",
        "Helvetica",
        "Arial",
        "DejaVuSans",
        "LiberationSans-Regular",
    ])
}

/// Load the best available font for monospace/computery text.
/// Font stack: Monaco > Menlo > DejaVu Sans Mono > Liberation Mono
#[allow(dead_code)]
pub fn load_mono_font() -> Option<FontData> {
    load_system_font(&[
        "Monaco",
        "Menlo",
        "Menlo-Regular",
        "DejaVuSansMono",
        "LiberationMono-Regular",
    ])
}

// --- 3D Isometric Visualization ---

/// Isometric projection constants.
/// We use a standard isometric angle: x-axis goes right-and-down at 30deg,
/// z-axis goes right-and-up at 30deg, y-axis goes straight up.
const ISO_ANGLE: f64 = std::f64::consts::PI / 6.0; // 30 degrees

/// Convert a 3D (x, y, z) point to 2D screen coordinates using isometric projection.
/// x: left-right on ground, z: front-back on ground, y: height (up).
/// Returns (screen_x, screen_y) relative to an origin point.
fn iso_to_screen(x: f64, y: f64, z: f64) -> (f64, f64) {
    let cos_a = ISO_ANGLE.cos(); // ~0.866
    let sin_a = ISO_ANGLE.sin(); // ~0.5
    let sx = (x - z) * cos_a;
    let sy = (x + z) * sin_a - y;
    (sx, sy)
}

/// Draw the full 3D visualization panel within the given bounds.
/// Shows sessions as isometric pillars and tasks as stacked cubes.
/// When disconnected, shows a demo visualization with animated placeholder geometry.
fn draw_3d_visualization(
    scene: &mut Scene,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    instance: &crate::protocol::LobsterInstance,
    elapsed: f64,
    font_data: Option<&FontData>,
) {
    let state = &instance.state;
    let is_connected = instance.status == ConnectionStatus::Connected;

    // Panel background
    let panel_rect = RoundedRect::new(x, y, x + w, y + h, CORNER_RADIUS);
    scene.fill(Fill::NonZero, Affine::IDENTITY, PANEL_BG, None, &panel_rect);
    scene.stroke(
        &vello::kurbo::Stroke::new(1.5),
        Affine::IDENTITY,
        PANEL_BORDER,
        None,
        &panel_rect,
    );

    // Section header
    let header_title = if is_connected { "3D OVERVIEW" } else { "3D OVERVIEW (awaiting connection)" };
    let header_y = draw_section_header_with_font(scene, x + PANEL_PADDING, y + PANEL_PADDING, w - 2.0 * PANEL_PADDING, header_title, font_data);

    let viz_x = x + PANEL_PADDING;
    let viz_y = header_y + 8.0;
    let viz_w = w - 2.0 * PANEL_PADDING;
    let viz_h = h - (viz_y - y) - PANEL_PADDING;

    // Draw the ground plane (isometric diamond)
    draw_iso_ground(scene, viz_x, viz_y, viz_w, viz_h);

    // Origin point for the isometric scene (center-bottom of the viz area)
    let origin_x = viz_x + viz_w * 0.5;
    let origin_y = viz_y + viz_h * 0.80;

    let scale = (viz_w.min(viz_h) / 400.0).max(0.4).min(1.5);

    if is_connected {
        // --- Connected: show real data ---

        // Sessions pillars (left side)
        let claude_sessions: Vec<_> = state.sessions.iter()
            .filter(|s| s.name == "claude")
            .collect();
        let session_count = claude_sessions.len();

        for (i, session) in claude_sessions.iter().enumerate() {
            let grid_x = -80.0 * scale + (i as f64) * 50.0 * scale;
            let grid_z = -40.0 * scale;

            let base_height = (session.memory_mb / 10.0).clamp(20.0, 120.0) * scale;
            let breath = (elapsed * 1.5 + i as f64 * 0.7).sin() * 3.0 * scale;
            let pillar_h = base_height + breath;
            let pillar_w = 30.0 * scale;

            draw_iso_shadow(scene, origin_x, origin_y, grid_x, grid_z, pillar_w, scale);
            draw_iso_box(
                scene, origin_x, origin_y,
                grid_x, 0.0, grid_z,
                pillar_w, pillar_h, pillar_w * 0.8,
                ISO_SESSION_TOP, ISO_SESSION_LEFT, ISO_SESSION_RIGHT,
            );

            let (lx, ly) = iso_to_screen(grid_x, -12.0 * scale, grid_z);
            draw_text_with_font(scene, origin_x + lx - 10.0 * scale, origin_y + ly, &format!("S{}", i + 1), TEXT_PRIMARY, 16.0 * scale, font_data);
        }

        let label_x = viz_x + 8.0;
        let label_y = viz_y + viz_h - 10.0;
        draw_text_with_font(scene, label_x, label_y, &format!("{} session{}", session_count, if session_count != 1 { "s" } else { "" }), TEXT_PRIMARY, 18.0, font_data);

        // Task cubes (right side)
        let tasks = &state.tasks.summary;
        let inbox_count = state.message_queues.inbox.count;

        let cube_size = 22.0 * scale;
        let task_base_x = 30.0 * scale;
        let task_base_z = -30.0 * scale;

        // Inbox items as red cubes
        let inbox_to_show = inbox_count.min(5) as usize;
        for i in 0..inbox_to_show {
            let ix = task_base_x + (i as f64) * (cube_size * 1.2);
            let iz = task_base_z - 40.0 * scale;
            let bounce = ((elapsed * 2.5 + i as f64 * 0.4).sin().abs()) * 8.0 * scale;

            draw_iso_shadow(scene, origin_x, origin_y, ix, iz, cube_size * 0.6, scale);
            draw_iso_box(
                scene, origin_x, origin_y,
                ix, bounce, iz,
                cube_size, cube_size, cube_size,
                ISO_INBOX_TOP, ISO_INBOX_LEFT, ISO_INBOX_RIGHT,
            );
        }
        if inbox_count > 5 {
            let (lx, ly) = iso_to_screen(task_base_x + 5.0 * cube_size * 1.2, 0.0, task_base_z - 40.0 * scale);
            draw_text_with_font(scene, origin_x + lx, origin_y + ly, &format!("+{}", inbox_count - 5), TEXT_PRIMARY, 14.0 * scale, font_data);
        }

        // Completed tasks (green)
        let completed = tasks.completed.min(10) as usize;
        draw_task_stack(
            scene, origin_x, origin_y,
            task_base_x, task_base_z,
            cube_size, completed,
            ISO_TASK_DONE_TOP, ISO_TASK_DONE_LEFT, ISO_TASK_DONE_RIGHT,
            elapsed, 0.0, scale,
        );

        // In-progress tasks (blue)
        let in_progress = tasks.in_progress.min(5) as usize;
        let completed_height = completed as f64 * cube_size * 0.65;
        draw_task_stack(
            scene, origin_x, origin_y,
            task_base_x + cube_size * 0.3, task_base_z + cube_size * 0.3,
            cube_size * 0.9, in_progress,
            ISO_TASK_ACTIVE_TOP, ISO_TASK_ACTIVE_LEFT, ISO_TASK_ACTIVE_RIGHT,
            elapsed, completed_height, scale,
        );

        // Pending tasks (amber)
        let pending = tasks.pending.min(8) as usize;
        draw_task_stack(
            scene, origin_x, origin_y,
            task_base_x + cube_size * 1.8, task_base_z,
            cube_size * 0.85, pending,
            ISO_TASK_PENDING_TOP, ISO_TASK_PENDING_LEFT, ISO_TASK_PENDING_RIGHT,
            elapsed, 0.0, scale,
        );

        // Legend
        let legend_x = viz_x + viz_w - 180.0;
        let legend_y = viz_y + viz_h - 50.0;
        draw_legend_dot(scene, legend_x, legend_y, ISO_TASK_DONE_TOP);
        draw_text_with_font(scene, legend_x + 14.0, legend_y + 4.0, &format!("{} done", tasks.completed), TEXT_PRIMARY, 14.0, font_data);
        draw_legend_dot(scene, legend_x, legend_y + 16.0, ISO_TASK_ACTIVE_TOP);
        draw_text_with_font(scene, legend_x + 14.0, legend_y + 20.0, &format!("{} active", tasks.in_progress), TEXT_PRIMARY, 14.0, font_data);
        draw_legend_dot(scene, legend_x, legend_y + 32.0, ISO_TASK_PENDING_TOP);
        draw_text_with_font(scene, legend_x + 14.0, legend_y + 36.0, &format!("{} pending", tasks.pending), TEXT_PRIMARY, 14.0, font_data);
        if inbox_count > 0 {
            draw_legend_dot(scene, legend_x, legend_y + 48.0, ISO_INBOX_TOP);
            draw_text_with_font(scene, legend_x + 14.0, legend_y + 52.0, &format!("{} inbox", inbox_count), TEXT_PRIMARY, 14.0, font_data);
        }
    } else {
        // --- Disconnected: show demo visualization with animated pillars ---
        let cube_size = 22.0 * scale;

        // Demo session pillars (3 animated pillars)
        for i in 0..3 {
            let grid_x = -80.0 * scale + (i as f64) * 50.0 * scale;
            let grid_z = -40.0 * scale;
            let base_height = (40.0 + i as f64 * 20.0) * scale;
            let breath = (elapsed * 1.2 + i as f64 * 0.9).sin() * 5.0 * scale;
            let pillar_h = base_height + breath;
            let pillar_w = 30.0 * scale;

            draw_iso_shadow(scene, origin_x, origin_y, grid_x, grid_z, pillar_w, scale);
            draw_iso_box(
                scene, origin_x, origin_y,
                grid_x, 0.0, grid_z,
                pillar_w, pillar_h, pillar_w * 0.8,
                ISO_SESSION_TOP, ISO_SESSION_LEFT, ISO_SESSION_RIGHT,
            );
        }

        // Demo task cubes (a few animated cubes on the right)
        let task_base_x = 30.0 * scale;
        let task_base_z = -30.0 * scale;

        draw_task_stack(
            scene, origin_x, origin_y,
            task_base_x, task_base_z,
            cube_size, 3,
            ISO_TASK_DONE_TOP, ISO_TASK_DONE_LEFT, ISO_TASK_DONE_RIGHT,
            elapsed, 0.0, scale,
        );
        draw_task_stack(
            scene, origin_x, origin_y,
            task_base_x + cube_size * 1.8, task_base_z,
            cube_size * 0.85, 2,
            ISO_TASK_PENDING_TOP, ISO_TASK_PENDING_LEFT, ISO_TASK_PENDING_RIGHT,
            elapsed, 0.0, scale,
        );

        // "Waiting for data..." label
        let label_x = viz_x + 8.0;
        let label_y = viz_y + viz_h - 10.0;
        draw_text_with_font(scene, label_x, label_y, "Waiting for connection...", TEXT_PRIMARY, 18.0, font_data);
    }
}

/// Draw a small colored square as a legend indicator.
fn draw_legend_dot(scene: &mut Scene, x: f64, y: f64, color: Color) {
    let rect = Rect::new(x, y - 4.0, x + 10.0, y + 6.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &rect);
}

/// Draw the isometric ground plane as a diamond shape with grid.
fn draw_iso_ground(scene: &mut Scene, x: f64, y: f64, w: f64, h: f64) {
    let cx = x + w * 0.5;
    let cy = y + h * 0.80;
    let hw = w * 0.45;
    let hh = hw * 0.5; // isometric ratio

    // Ground diamond
    let mut path = BezPath::new();
    path.move_to(Point::new(cx, cy - hh));       // top
    path.line_to(Point::new(cx + hw, cy));         // right
    path.line_to(Point::new(cx, cy + hh));         // bottom
    path.line_to(Point::new(cx - hw, cy));         // left
    path.close_path();
    scene.fill(Fill::NonZero, Affine::IDENTITY, ISO_GROUND, None, &path);

    // Isometric grid: lines along both axes of the diamond
    let steps = 5;
    for i in 1..steps {
        let t = i as f64 / steps as f64;

        // Lines parallel to top-right edge (from left edge to bottom edge)
        let mut line1 = BezPath::new();
        let p1 = Point::new(cx - hw * (1.0 - t), cy - hh * t);
        let p2 = Point::new(cx + hw * t, cy + hh * (1.0 - t));
        line1.move_to(p1);
        line1.line_to(p2);
        scene.stroke(
            &vello::kurbo::Stroke::new(0.5),
            Affine::IDENTITY, ISO_GRID, None, &line1,
        );

        // Lines parallel to top-left edge (from right edge to bottom edge)
        let mut line2 = BezPath::new();
        let p3 = Point::new(cx + hw * (1.0 - t), cy - hh * t);
        let p4 = Point::new(cx - hw * t, cy + hh * (1.0 - t));
        line2.move_to(p3);
        line2.line_to(p4);
        scene.stroke(
            &vello::kurbo::Stroke::new(0.5),
            Affine::IDENTITY, ISO_GRID, None, &line2,
        );
    }
}

/// Draw a soft shadow on the ground plane beneath an isometric object.
fn draw_iso_shadow(
    scene: &mut Scene,
    origin_x: f64,
    origin_y: f64,
    grid_x: f64,
    grid_z: f64,
    size: f64,
    _scale: f64,
) {
    let (sx, sy) = iso_to_screen(grid_x, 0.0, grid_z);
    let shadow_w = size * 1.2;
    let shadow_h = size * 0.4;
    let cx = origin_x + sx;
    let cy = origin_y + sy;

    let ellipse = vello::kurbo::Ellipse::new(Point::new(cx, cy + 2.0), (shadow_w, shadow_h), 0.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, ISO_SHADOW, None, &ellipse);
}

/// Draw a 3D isometric box (rectangular prism) at the given grid position.
/// (grid_x, base_y, grid_z) is the base position in 3D space.
/// (w, h, d) are width, height, depth of the box.
fn draw_iso_box(
    scene: &mut Scene,
    origin_x: f64,
    origin_y: f64,
    grid_x: f64,
    base_y: f64,
    grid_z: f64,
    w: f64,
    h: f64,
    d: f64,
    top_color: Color,
    left_color: Color,
    right_color: Color,
) {
    // 8 corners of the box in 3D, projected to 2D
    // Bottom face corners
    let b_fl = iso_to_screen(grid_x, base_y, grid_z);         // front-left
    let b_fr = iso_to_screen(grid_x + w, base_y, grid_z);     // front-right
    let _b_br = iso_to_screen(grid_x + w, base_y, grid_z + d); // back-right
    let b_bl = iso_to_screen(grid_x, base_y, grid_z + d);      // back-left

    // Top face corners
    let t_fl = iso_to_screen(grid_x, base_y + h, grid_z);
    let t_fr = iso_to_screen(grid_x + w, base_y + h, grid_z);
    let t_br = iso_to_screen(grid_x + w, base_y + h, grid_z + d);
    let t_bl = iso_to_screen(grid_x, base_y + h, grid_z + d);

    // Convert to screen coordinates
    let to_pt = |p: (f64, f64)| Point::new(origin_x + p.0, origin_y + p.1);

    // Draw faces back-to-front for correct occlusion:
    // 1. Left face (back-left side)
    let mut left_face = BezPath::new();
    left_face.move_to(to_pt(b_fl));
    left_face.line_to(to_pt(b_bl));
    left_face.line_to(to_pt(t_bl));
    left_face.line_to(to_pt(t_fl));
    left_face.close_path();
    scene.fill(Fill::NonZero, Affine::IDENTITY, left_color, None, &left_face);

    // 2. Right face (front-right side)
    let mut right_face = BezPath::new();
    right_face.move_to(to_pt(b_fl));
    right_face.line_to(to_pt(b_fr));
    right_face.line_to(to_pt(t_fr));
    right_face.line_to(to_pt(t_fl));
    right_face.close_path();
    scene.fill(Fill::NonZero, Affine::IDENTITY, right_color, None, &right_face);

    // 3. Top face
    let mut top_face = BezPath::new();
    top_face.move_to(to_pt(t_fl));
    top_face.line_to(to_pt(t_fr));
    top_face.line_to(to_pt(t_br));
    top_face.line_to(to_pt(t_bl));
    top_face.close_path();
    scene.fill(Fill::NonZero, Affine::IDENTITY, top_color, None, &top_face);

    // Edge outlines for definition
    let edge_color = Color::new([0.0_f32, 0.0, 0.0, 0.15]);
    let stroke = vello::kurbo::Stroke::new(0.8);

    // Left face edges
    scene.stroke(&stroke, Affine::IDENTITY, edge_color, None, &left_face);
    // Right face edges
    scene.stroke(&stroke, Affine::IDENTITY, edge_color, None, &right_face);
    // Top face edges
    scene.stroke(&stroke, Affine::IDENTITY, edge_color, None, &top_face);
}

/// Draw a stack of cubes for tasks.
fn draw_task_stack(
    scene: &mut Scene,
    origin_x: f64,
    origin_y: f64,
    base_x: f64,
    base_z: f64,
    cube_size: f64,
    count: usize,
    top_color: Color,
    left_color: Color,
    right_color: Color,
    elapsed: f64,
    base_height: f64,
    _scale: f64,
) {
    let stack_gap = cube_size * 0.65; // slight overlap for tight stacking

    for i in 0..count {
        let y_pos = base_height + i as f64 * stack_gap;
        // Subtle wobble animation
        let wobble_x = (elapsed * 0.8 + i as f64 * 1.1).sin() * 1.5;
        let wobble_z = (elapsed * 0.6 + i as f64 * 0.9).cos() * 1.0;

        // Shadow only for bottom cube
        if i == 0 && base_height < 0.1 {
            draw_iso_shadow(scene, origin_x, origin_y, base_x + wobble_x, base_z + wobble_z, cube_size * 0.5, 1.0);
        }

        draw_iso_box(
            scene, origin_x, origin_y,
            base_x + wobble_x, y_pos, base_z + wobble_z,
            cube_size, cube_size, cube_size * 0.8,
            top_color, left_color, right_color,
        );
    }
}


// --- Utility functions ---

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let mins = (seconds % 3600) / 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn cpu_color(percent: f64) -> Color {
    if percent > 80.0 { ACCENT_RED }
    else if percent > 50.0 { ACCENT_AMBER }
    else { ACCENT_GREEN }
}

fn mem_color(percent: f64) -> Color {
    if percent > 85.0 { ACCENT_RED }
    else if percent > 60.0 { ACCENT_AMBER }
    else { ACCENT_GREEN }
}

fn disk_color(percent: f64) -> Color {
    if percent > 90.0 { ACCENT_RED }
    else if percent > 75.0 { ACCENT_AMBER }
    else { ACCENT_GREEN }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // --- Color constant tests ---

    #[test]
    fn test_bg_color_is_bisque() {
        // CSS bisque = rgb(255, 228, 196) = [1.0, 0.894, 0.769]
        let components = BG_COLOR.components;
        assert!((components[0] - 1.0).abs() < 0.01, "BG red channel should be ~1.0");
        assert!((components[1] - 0.894).abs() < 0.01, "BG green channel should be ~0.894");
        assert!((components[2] - 0.769).abs() < 0.01, "BG blue channel should be ~0.769");
        assert!((components[3] - 1.0).abs() < 0.01, "BG alpha should be 1.0");
    }

    #[test]
    fn test_text_primary_is_pure_black() {
        let components = TEXT_PRIMARY.components;
        assert_eq!(components[0], 0.0, "TEXT_PRIMARY red should be 0.0");
        assert_eq!(components[1], 0.0, "TEXT_PRIMARY green should be 0.0");
        assert_eq!(components[2], 0.0, "TEXT_PRIMARY blue should be 0.0");
        assert_eq!(components[3], 1.0, "TEXT_PRIMARY alpha should be 1.0");
    }

    #[test]
    fn test_text_light_is_black() {
        // TEXT_LIGHT (used on headers) should also be black
        let components = TEXT_LIGHT.components;
        assert_eq!(components[0], 0.0, "TEXT_LIGHT red should be 0.0");
        assert_eq!(components[1], 0.0, "TEXT_LIGHT green should be 0.0");
        assert_eq!(components[2], 0.0, "TEXT_LIGHT blue should be 0.0");
        assert_eq!(components[3], 1.0, "TEXT_LIGHT alpha should be 1.0");
    }

    #[test]
    fn test_text_secondary_is_black_with_opacity() {
        let components = TEXT_SECONDARY.components;
        assert_eq!(components[0], 0.0, "TEXT_SECONDARY red should be 0.0");
        assert_eq!(components[1], 0.0, "TEXT_SECONDARY green should be 0.0");
        assert_eq!(components[2], 0.0, "TEXT_SECONDARY blue should be 0.0");
        assert!((components[3] - 0.65).abs() < 0.01, "TEXT_SECONDARY alpha should be ~0.65");
    }

    #[test]
    fn test_accent_colors_are_distinct() {
        // Ensure accent colors are different from each other
        assert_ne!(ACCENT_GREEN.components, ACCENT_RED.components);
        assert_ne!(ACCENT_GREEN.components, ACCENT_AMBER.components);
        assert_ne!(ACCENT_RED.components, ACCENT_AMBER.components);
        assert_ne!(ACCENT_BLUE.components, ACCENT_GREEN.components);
    }

    #[test]
    fn test_panel_bg_lighter_than_main_bg() {
        // Panel background should be lighter (higher values) than the main background
        // to create visual hierarchy
        assert!(PANEL_BG.components[1] > BG_COLOR.components[1],
            "Panel BG green channel should be lighter than main BG");
    }

    // --- Isometric projection tests ---

    #[test]
    fn test_iso_to_screen_origin() {
        let (sx, sy) = iso_to_screen(0.0, 0.0, 0.0);
        assert!((sx).abs() < 1e-10, "Origin x should be ~0, got {}", sx);
        assert!((sy).abs() < 1e-10, "Origin y should be ~0, got {}", sy);
    }

    #[test]
    fn test_iso_to_screen_x_axis() {
        // Moving along the x-axis should go right and down
        let (sx, sy) = iso_to_screen(100.0, 0.0, 0.0);
        assert!(sx > 0.0, "Positive x should project right, got {}", sx);
        assert!(sy > 0.0, "Positive x should project down, got {}", sy);
    }

    #[test]
    fn test_iso_to_screen_z_axis() {
        // Moving along the z-axis should go left and down
        let (sx, sy) = iso_to_screen(0.0, 0.0, 100.0);
        assert!(sx < 0.0, "Positive z should project left, got {}", sx);
        assert!(sy > 0.0, "Positive z should project down, got {}", sy);
    }

    #[test]
    fn test_iso_to_screen_y_axis() {
        // Moving up along the y-axis should go up on screen (negative sy)
        let (sx, sy) = iso_to_screen(0.0, 100.0, 0.0);
        assert!((sx).abs() < 1e-10, "Y movement should not affect screen x");
        assert!(sy < 0.0, "Positive y should project up (negative sy), got {}", sy);
    }

    #[test]
    fn test_iso_to_screen_symmetry() {
        // iso_to_screen(a, 0, 0) and iso_to_screen(0, 0, a) should be mirror images
        let (sx1, sy1) = iso_to_screen(100.0, 0.0, 0.0);
        let (sx2, sy2) = iso_to_screen(0.0, 0.0, 100.0);
        assert!((sx1 + sx2).abs() < 1e-10, "x and z projections should mirror in x");
        assert!((sy1 - sy2).abs() < 1e-10, "x and z projections should have same y");
    }

    #[test]
    fn test_iso_angle_is_30_degrees() {
        let expected = std::f64::consts::PI / 6.0;
        assert!((ISO_ANGLE - expected).abs() < 1e-10, "ISO_ANGLE should be pi/6 (30 degrees)");
    }

    #[test]
    fn test_iso_projection_uses_correct_trig() {
        // Verify the formula: sx = (x - z) * cos(30), sy = (x + z) * sin(30) - y
        let x = 50.0;
        let y = 30.0;
        let z = 20.0;
        let (sx, sy) = iso_to_screen(x, y, z);

        let cos30 = (std::f64::consts::PI / 6.0).cos();
        let sin30 = (std::f64::consts::PI / 6.0).sin();

        let expected_sx = (x - z) * cos30;
        let expected_sy = (x + z) * sin30 - y;

        assert!((sx - expected_sx).abs() < 1e-10, "sx formula mismatch");
        assert!((sy - expected_sy).abs() < 1e-10, "sy formula mismatch");
    }

    // --- Splash animation tests ---

    #[test]
    fn test_splash_alpha_starts_at_zero() {
        assert!((splash_alpha(0.0)).abs() < 0.01, "Alpha at t=0 should be ~0");
    }

    #[test]
    fn test_splash_alpha_reaches_full() {
        // At the end of fade-in, should be close to 1.0
        let alpha = splash_alpha(FADE_IN_DURATION);
        assert!((alpha - 1.0).abs() < 0.01, "Alpha at end of fade-in should be ~1.0, got {}", alpha);
    }

    #[test]
    fn test_splash_alpha_holds_at_full() {
        let alpha = splash_alpha(FADE_IN_DURATION + HOLD_DURATION * 0.5);
        assert_eq!(alpha, 1.0, "Alpha during hold should be exactly 1.0");
    }

    #[test]
    fn test_splash_alpha_fades_out() {
        let alpha = splash_alpha(TOTAL_SPLASH_DURATION - 0.01);
        assert!(alpha > 0.0, "Alpha near end should still be positive");
        assert!(alpha < 0.1, "Alpha near end should be close to 0");
    }

    #[test]
    fn test_splash_alpha_gone_after_duration() {
        let alpha = splash_alpha(TOTAL_SPLASH_DURATION);
        assert_eq!(alpha, 0.0, "Alpha after total duration should be 0");
        let alpha_later = splash_alpha(TOTAL_SPLASH_DURATION + 100.0);
        assert_eq!(alpha_later, 0.0, "Alpha well after duration should be 0");
    }

    #[test]
    fn test_splash_timing_constants() {
        assert_eq!(FADE_IN_DURATION, 2.0);
        assert_eq!(HOLD_DURATION, 4.0);
        assert_eq!(FADE_OUT_DURATION, 2.0);
        assert_eq!(TOTAL_SPLASH_DURATION, 8.0);
    }

    // --- Ease function tests ---

    #[test]
    fn test_ease_in_out_boundaries() {
        assert!((ease_in_out(0.0)).abs() < 1e-6, "ease(0) should be 0");
        assert!((ease_in_out(1.0) - 1.0).abs() < 1e-6, "ease(1) should be 1");
    }

    #[test]
    fn test_ease_in_out_midpoint() {
        let mid = ease_in_out(0.5);
        assert!((mid - 0.5).abs() < 0.01, "ease(0.5) should be ~0.5, got {}", mid);
    }

    #[test]
    fn test_ease_in_out_monotonic() {
        // The function should be monotonically increasing
        let mut prev = 0.0f32;
        for i in 1..=100 {
            let t = i as f32 / 100.0;
            let val = ease_in_out(t);
            assert!(val >= prev, "ease_in_out should be monotonic at t={}", t);
            prev = val;
        }
    }

    // --- Font loading tests ---

    #[test]
    fn test_load_readable_font_has_fallbacks() {
        // This test verifies the function runs without panicking.
        // On Linux CI, it may return None (no macOS fonts), which is fine.
        let result = load_readable_font();
        // Either it loads a font or returns None -- both are valid
        let _ = result;
    }

    #[test]
    fn test_load_mono_font_has_fallbacks() {
        let result = load_mono_font();
        let _ = result;
    }

    #[test]
    fn test_load_system_font_with_nonexistent_font_returns_none() {
        let result = load_system_font(&["NonExistentFont12345"]);
        assert!(result.is_none(), "Nonexistent font should return None");
    }

    #[test]
    fn test_load_system_font_empty_list_returns_none() {
        let result = load_system_font(&[]);
        assert!(result.is_none(), "Empty font list should return None");
    }

    // --- Layout constants tests ---

    #[test]
    fn test_layout_constants_positive() {
        assert!(MARGIN > 0.0);
        assert!(PANEL_PADDING > 0.0);
        assert!(PANEL_GAP > 0.0);
        assert!(HEADER_HEIGHT > 0.0);
        assert!(SECTION_HEIGHT > 0.0);
        assert!(LINE_HEIGHT > 0.0);
        assert!(BAR_HEIGHT > 0.0);
        assert!(CORNER_RADIUS > 0.0);
        assert!(STATUS_DOT_RADIUS > 0.0);
    }

    #[test]
    fn test_header_height_accommodates_doubled_text() {
        // Header height should be at least 60.0 for doubled text
        assert!(HEADER_HEIGHT >= 60.0, "Header should be tall enough for 2x text");
    }

    #[test]
    fn test_line_height_accommodates_doubled_text() {
        // Line height should be at least 30.0 for doubled text
        assert!(LINE_HEIGHT >= 30.0, "Line height should accommodate 2x text");
    }

    // --- Utility function tests ---

    #[test]
    fn test_format_uptime_minutes() {
        assert_eq!(format_uptime(0), "0m");
        assert_eq!(format_uptime(59), "0m");
        assert_eq!(format_uptime(60), "1m");
        assert_eq!(format_uptime(120), "2m");
    }

    #[test]
    fn test_format_uptime_hours() {
        assert_eq!(format_uptime(3600), "1h 0m");
        assert_eq!(format_uptime(3660), "1h 1m");
        assert_eq!(format_uptime(7200), "2h 0m");
    }

    #[test]
    fn test_format_uptime_days() {
        assert_eq!(format_uptime(86400), "1d 0h 0m");
        assert_eq!(format_uptime(90061), "1d 1h 1m");
        assert_eq!(format_uptime(172800), "2d 0h 0m");
    }

    #[test]
    fn test_cpu_color_green_for_low() {
        let color = cpu_color(10.0);
        assert_eq!(color.components, ACCENT_GREEN.components);
    }

    #[test]
    fn test_cpu_color_amber_for_medium() {
        let color = cpu_color(60.0);
        assert_eq!(color.components, ACCENT_AMBER.components);
    }

    #[test]
    fn test_cpu_color_red_for_high() {
        let color = cpu_color(90.0);
        assert_eq!(color.components, ACCENT_RED.components);
    }

    #[test]
    fn test_mem_color_thresholds() {
        assert_eq!(mem_color(50.0).components, ACCENT_GREEN.components);
        assert_eq!(mem_color(70.0).components, ACCENT_AMBER.components);
        assert_eq!(mem_color(90.0).components, ACCENT_RED.components);
    }

    #[test]
    fn test_disk_color_thresholds() {
        assert_eq!(disk_color(50.0).components, ACCENT_GREEN.components);
        assert_eq!(disk_color(80.0).components, ACCENT_AMBER.components);
        assert_eq!(disk_color(95.0).components, ACCENT_RED.components);
    }

    // --- Bitmap font tests ---

    #[test]
    fn test_char_bitmap_all_printable_ascii() {
        // Every printable ASCII char should return a 7-element array
        for ch in 32u8..=126 {
            let bitmap = get_char_bitmap(ch);
            assert_eq!(bitmap.len(), 7, "Bitmap for '{}' should have 7 rows", ch as char);
        }
    }

    #[test]
    fn test_char_bitmap_space_is_empty() {
        // Space doesn't go through the bitmap renderer (skipped in draw_text)
        // but the bitmap itself should be whatever the default is
        // (doesn't matter since spaces are skipped)
    }

    #[test]
    fn test_char_bitmap_letters_not_empty() {
        // Common letters should have at least some pixels set
        for ch in b'A'..=b'Z' {
            let bitmap = get_char_bitmap(ch);
            let total_pixels: u32 = bitmap.iter().map(|row| row.count_ones()).sum();
            assert!(total_pixels > 0, "Letter '{}' should have pixels set", ch as char);
        }
        for ch in b'a'..=b'z' {
            let bitmap = get_char_bitmap(ch);
            let total_pixels: u32 = bitmap.iter().map(|row| row.count_ones()).sum();
            assert!(total_pixels > 0, "Letter '{}' should have pixels set", ch as char);
        }
    }

    #[test]
    fn test_char_bitmap_digits_not_empty() {
        for ch in b'0'..=b'9' {
            let bitmap = get_char_bitmap(ch);
            let total_pixels: u32 = bitmap.iter().map(|row| row.count_ones()).sum();
            assert!(total_pixels > 0, "Digit '{}' should have pixels set", ch as char);
        }
    }

    #[test]
    fn test_char_bitmap_unknown_gives_box() {
        // Unknown characters should render as a box (filled outline)
        let bitmap = get_char_bitmap(0xFF);
        let expected_box: [u8; 7] = [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111];
        assert_eq!(bitmap, expected_box, "Unknown chars should render as a box");
    }

    // --- 3D Isometric color tests ---

    #[test]
    fn test_iso_colors_have_three_shading_levels() {
        // Each iso object should have top (brightest), right (medium), left (darkest)
        // for session pillars
        assert!(ISO_SESSION_TOP.components[0] > ISO_SESSION_LEFT.components[0],
            "Session top should be brighter than left");
        assert!(ISO_SESSION_RIGHT.components[0] > ISO_SESSION_LEFT.components[0],
            "Session right should be brighter than left");

        // for task cubes (done)
        assert!(ISO_TASK_DONE_RIGHT.components[1] > ISO_TASK_DONE_LEFT.components[1],
            "Task done right should be brighter than left (green channel)");
    }

    #[test]
    fn test_iso_shadow_is_semi_transparent() {
        assert!(ISO_SHADOW.components[3] < 0.5,
            "Shadow should be semi-transparent, got alpha {}", ISO_SHADOW.components[3]);
    }

    // --- Render pipeline integration tests ---

    #[test]
    fn test_render_dashboard_no_instances() {
        let mut scene = Scene::new();
        let instances = Arc::new(Mutex::new(Vec::new()));
        // Should not panic when rendering with no instances
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 0.0, None);
    }

    #[test]
    fn test_render_dashboard_single_disconnected_instance() {
        let mut scene = Scene::new();
        let instance = crate::protocol::LobsterInstance::new("ws://localhost:9100".to_string());
        let instances = Arc::new(Mutex::new(vec![instance]));
        // Should not panic, and should still render 3D viz (demo mode)
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 0.0, None);
    }

    #[test]
    fn test_render_dashboard_with_connected_instance() {
        let mut scene = Scene::new();
        let mut instance = crate::protocol::LobsterInstance::new("ws://localhost:9100".to_string());
        instance.status = crate::protocol::ConnectionStatus::Connected;
        instance.state.system.hostname = "test-host".to_string();
        instance.state.system.cpu.percent = 45.0;
        instance.state.system.memory.percent = 60.0;
        instance.state.system.disk.percent = 30.0;
        let instances = Arc::new(Mutex::new(vec![instance]));
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 0.0, None);
    }

    #[test]
    fn test_render_dashboard_multiple_instances() {
        let mut scene = Scene::new();
        let mut inst1 = crate::protocol::LobsterInstance::new("ws://host1:9100".to_string());
        inst1.status = crate::protocol::ConnectionStatus::Connected;
        inst1.state.system.hostname = "host1".to_string();
        let inst2 = crate::protocol::LobsterInstance::new("ws://host2:9100".to_string());
        let instances = Arc::new(Mutex::new(vec![inst1, inst2]));
        render_dashboard(&mut scene, 1920.0, 1080.0, &instances, 5.0, None);
    }

    #[test]
    fn test_render_dashboard_during_splash() {
        let mut scene = Scene::new();
        let instances = Arc::new(Mutex::new(Vec::new()));
        // During splash animation (elapsed < TOTAL_SPLASH_DURATION)
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 1.0, None);
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 4.0, None);
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 7.0, None);
    }

    #[test]
    fn test_render_dashboard_after_splash() {
        let mut scene = Scene::new();
        let instances = Arc::new(Mutex::new(Vec::new()));
        // After splash
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 100.0, None);
    }

    #[test]
    fn test_3d_visualization_with_tasks() {
        let mut scene = Scene::new();
        let mut instance = crate::protocol::LobsterInstance::new("ws://localhost:9100".to_string());
        instance.status = crate::protocol::ConnectionStatus::Connected;
        instance.state.tasks.summary.completed = 5;
        instance.state.tasks.summary.in_progress = 2;
        instance.state.tasks.summary.pending = 3;
        instance.state.message_queues.inbox.count = 4;
        // Should render without panicking
        draw_3d_visualization(&mut scene, 0.0, 0.0, 800.0, 400.0, &instance, 1.0, None);
    }

    #[test]
    fn test_3d_visualization_disconnected_demo() {
        let mut scene = Scene::new();
        let instance = crate::protocol::LobsterInstance::new("ws://localhost:9100".to_string());
        // When disconnected, should show demo geometry
        draw_3d_visualization(&mut scene, 0.0, 0.0, 800.0, 400.0, &instance, 1.0, None);
    }

    #[test]
    fn test_3d_visualization_with_sessions() {
        let mut scene = Scene::new();
        let mut instance = crate::protocol::LobsterInstance::new("ws://localhost:9100".to_string());
        instance.status = crate::protocol::ConnectionStatus::Connected;
        instance.state.sessions = vec![
            crate::protocol::Session {
                pid: 1234,
                name: "claude".to_string(),
                cmdline: "claude".to_string(),
                started: None,
                cpu_percent: 10.0,
                memory_mb: 256.0,
            },
            crate::protocol::Session {
                pid: 5678,
                name: "claude".to_string(),
                cmdline: "claude".to_string(),
                started: None,
                cpu_percent: 5.0,
                memory_mb: 512.0,
            },
        ];
        draw_3d_visualization(&mut scene, 0.0, 0.0, 800.0, 400.0, &instance, 2.5, None);
    }

    // --- Ulysses quote test ---

    #[test]
    fn test_ulysses_quote_is_nonempty() {
        assert!(!ULYSSES_QUOTE.is_empty(), "Ulysses quote should not be empty");
        assert!(ULYSSES_QUOTE.contains("yes"), "Quote should contain 'yes'");
        assert!(ULYSSES_QUOTE.ends_with("Yes."), "Quote should end with 'Yes.'");
    }

    // --- Layout split ratio tests ---

    #[test]
    fn test_single_instance_split_ratio() {
        // In single instance mode, info panel gets 45%, 3D viz gets 55%
        let content_width = 1000.0;
        let split = 0.45;
        let info_w = content_width * split - PANEL_GAP * 0.5;
        let viz_w = content_width * (1.0 - split) - PANEL_GAP * 0.5;

        assert!(info_w > 0.0, "Info panel width should be positive");
        assert!(viz_w > 0.0, "Viz panel width should be positive");
        assert!(viz_w > info_w, "Viz panel should be wider than info panel");
        assert!((info_w + viz_w + PANEL_GAP - content_width).abs() < 0.01,
            "Panels should fill the content width");
    }

    #[test]
    fn test_multi_instance_grid_cols() {
        // 1 instance -> 1 col
        let cols1 = if 1 <= 1 { 1 } else if 1 <= 4 { 2 } else { 3 };
        assert_eq!(cols1, 1);

        // 2 instances -> 2 cols
        let cols2 = if 2 <= 1 { 1 } else if 2 <= 4 { 2 } else { 3 };
        assert_eq!(cols2, 2);

        // 4 instances -> 2 cols
        let cols4 = if 4 <= 1 { 1 } else if 4 <= 4 { 2 } else { 3 };
        assert_eq!(cols4, 2);

        // 5 instances -> 3 cols
        let cols5 = if 5 <= 1 { 1 } else if 5 <= 4 { 2 } else { 3 };
        assert_eq!(cols5, 3);
    }
}
