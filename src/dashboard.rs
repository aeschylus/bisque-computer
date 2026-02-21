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
pub fn render_dashboard(
    scene: &mut Scene,
    width: f64,
    height: f64,
    instances: &SharedInstances,
    elapsed: f64,
    font_data: Option<&FontData>,
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
        draw_centered_text(scene, width, height, "No Lobster instances configured", TEXT_SECONDARY, 40.0);
        return;
    }

    // Title bar
    let title_rect = Rect::new(0.0, 0.0, width, HEADER_HEIGHT);
    scene.fill(Fill::NonZero, Affine::IDENTITY, HEADER_BG, None, &title_rect);
    draw_text(scene, MARGIN, 50.0, "LOBSTER DASHBOARD", TEXT_LIGHT, 44.0);

    // Count connected instances
    let connected = instances
        .iter()
        .filter(|i| i.status == ConnectionStatus::Connected)
        .count();
    let status_text = format!("{}/{} connected", connected, instances.len());
    draw_text(scene, width - 400.0, 50.0, &status_text, TEXT_LIGHT, 28.0);

    // Layout: panels in a grid with 3D visualization
    let content_top = HEADER_HEIGHT + MARGIN;
    let content_width = width - 2.0 * MARGIN;
    let content_height = height - content_top - MARGIN;

    let num_instances = instances.len();

    // Check if any instance is connected (for 3D viz)
    let any_connected = instances.iter().any(|i| i.status == ConnectionStatus::Connected);

    if num_instances == 1 && any_connected {
        // Single instance: side-by-side layout — info panel left, 3D viz right
        let split = 0.45; // info panel gets 45% width, 3D viz gets 55%
        let info_w = content_width * split - PANEL_GAP * 0.5;
        let viz_w = content_width * (1.0 - split) - PANEL_GAP * 0.5;

        draw_instance_panel(scene, MARGIN, content_top, info_w, content_height, &instances[0]);
        draw_3d_visualization(
            scene,
            MARGIN + info_w + PANEL_GAP, content_top,
            viz_w, content_height,
            &instances[0], elapsed,
        );
    } else {
        // Multiple instances: grid of info panels + 3D viz row at bottom
        let viz_height = if any_connected { content_height * 0.35 } else { 0.0 };
        let panels_height = content_height - viz_height - if any_connected { PANEL_GAP } else { 0.0 };

        let cols = if num_instances <= 1 { 1 } else if num_instances <= 4 { 2 } else { 3 };
        let rows = (num_instances + cols - 1) / cols;

        let panel_width = (content_width - (cols as f64 - 1.0) * PANEL_GAP) / cols as f64;
        let panel_height = (panels_height - (rows as f64 - 1.0) * PANEL_GAP) / rows as f64;

        for (idx, instance) in instances.iter().enumerate() {
            let col = idx % cols;
            let row = idx / cols;
            let x = MARGIN + col as f64 * (panel_width + PANEL_GAP);
            let y = content_top + row as f64 * (panel_height + PANEL_GAP);

            draw_instance_panel(scene, x, y, panel_width, panel_height, instance);
        }

        // Draw 3D viz panels for connected instances along the bottom
        if any_connected {
            let viz_top = content_top + panels_height + PANEL_GAP;
            let connected_instances: Vec<_> = instances.iter()
                .filter(|i| i.status == ConnectionStatus::Connected)
                .collect();
            let viz_cols = connected_instances.len();
            let viz_panel_w = (content_width - (viz_cols as f64 - 1.0).max(0.0) * PANEL_GAP) / viz_cols as f64;

            for (idx, instance) in connected_instances.iter().enumerate() {
                let vx = MARGIN + idx as f64 * (viz_panel_w + PANEL_GAP);
                draw_3d_visualization(scene, vx, viz_top, viz_panel_w, viz_height, instance, elapsed);
            }
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
    draw_text(scene, inner_x + 24.0, cursor_y + 20.0, hostname, TEXT_PRIMARY, 32.0);

    // Connection status text
    let status_str = match &instance.status {
        ConnectionStatus::Connected => "connected".to_string(),
        ConnectionStatus::Connecting => "connecting...".to_string(),
        ConnectionStatus::Disconnected => "disconnected".to_string(),
        ConnectionStatus::Error(e) => format!("error: {}", &e[..e.len().min(30)]),
    };
    draw_text(scene, inner_x + 24.0, cursor_y + 48.0, &status_str, TEXT_SECONDARY, 22.0);
    cursor_y += 60.0;

    if instance.status != ConnectionStatus::Connected {
        return;
    }

    let state = &instance.state;

    // --- System section ---
    cursor_y = draw_section_header(scene, inner_x, cursor_y, inner_w, "SYSTEM");

    // Uptime
    let uptime_str = format_uptime(state.system.uptime_seconds);
    draw_label_value(scene, inner_x, cursor_y, "Uptime", &uptime_str, inner_w);
    cursor_y += LINE_HEIGHT;

    // CPU bar
    draw_label_bar(scene, inner_x, cursor_y, "CPU", state.system.cpu.percent, inner_w, cpu_color(state.system.cpu.percent));
    cursor_y += LINE_HEIGHT;

    // Memory bar
    draw_label_bar(scene, inner_x, cursor_y, "Memory", state.system.memory.percent, inner_w, mem_color(state.system.memory.percent));
    cursor_y += LINE_HEIGHT;

    // Disk bar
    draw_label_bar(scene, inner_x, cursor_y, "Disk", state.system.disk.percent, inner_w, disk_color(state.system.disk.percent));
    cursor_y += LINE_HEIGHT + 4.0;

    // --- Sessions section ---
    cursor_y = draw_section_header(scene, inner_x, cursor_y, inner_w, "SESSIONS");

    // Filter to just actual Claude processes (not wrapper scripts, tmux, etc.)
    let claude_sessions: Vec<_> = state.sessions.iter()
        .filter(|s| s.name == "claude")
        .collect();
    let session_count = claude_sessions.len();
    draw_label_value(scene, inner_x, cursor_y, "Active", &format!("{}", session_count), inner_w);
    cursor_y += LINE_HEIGHT;

    // Show memory for each session (up to 3)
    for (_i, session) in claude_sessions.iter().take(3).enumerate() {
        let label = format!("  PID {}", session.pid);
        let value = format!("{:.0} MB", session.memory_mb);
        draw_label_value(scene, inner_x, cursor_y, &label, &value, inner_w);
        cursor_y += LINE_HEIGHT;
    }
    if session_count > 3 {
        draw_text(scene, inner_x + 8.0, cursor_y, &format!("  +{} more", session_count - 3), TEXT_SECONDARY, 22.0);
        cursor_y += LINE_HEIGHT;
    }
    cursor_y += 4.0;

    // --- Messages section ---
    cursor_y = draw_section_header(scene, inner_x, cursor_y, inner_w, "MESSAGES");

    let queues = &state.message_queues;
    let queue_items = [
        ("Inbox", queues.inbox.count, if queues.inbox.count > 0 { ACCENT_AMBER } else { ACCENT_GREEN }),
        ("Processed", queues.processed.count, TEXT_SECONDARY),
        ("Sent", queues.sent.count, ACCENT_BLUE),
        ("Failed", queues.failed.count, if queues.failed.count > 0 { ACCENT_RED } else { TEXT_SECONDARY }),
    ];

    for (label, count, color) in &queue_items {
        draw_label_value_colored(scene, inner_x, cursor_y, label, &count.to_string(), inner_w, *color);
        cursor_y += LINE_HEIGHT;
    }
    cursor_y += 4.0;

    // --- Activity section ---
    if cursor_y + 60.0 < y + h {
        cursor_y = draw_section_header(scene, inner_x, cursor_y, inner_w, "ACTIVITY (24h)");

        let activity = &state.conversation_activity;
        draw_label_value(scene, inner_x, cursor_y, "Received", &activity.messages_received_24h.to_string(), inner_w);
        cursor_y += LINE_HEIGHT;
        draw_label_value(scene, inner_x, cursor_y, "Replied", &activity.replies_sent_24h.to_string(), inner_w);
        cursor_y += LINE_HEIGHT + 4.0;
    }

    // --- Health section ---
    if cursor_y + 40.0 < y + h {
        cursor_y = draw_section_header(scene, inner_x, cursor_y, inner_w, "HEALTH");

        let health = &state.health;
        let hb_status = match health.heartbeat_age_seconds {
            Some(age) if age < 300 => format!("{}s ago", age),
            Some(age) => format!("STALE ({}s)", age),
            None => "unknown".to_string(),
        };
        let hb_color = if health.heartbeat_stale { ACCENT_RED } else { ACCENT_GREEN };
        draw_label_value_colored(scene, inner_x, cursor_y, "Heartbeat", &hb_status, inner_w, hb_color);
        cursor_y += LINE_HEIGHT;

        let bot_status = if health.telegram_bot_running { "running" } else { "stopped" };
        let bot_color = if health.telegram_bot_running { ACCENT_GREEN } else { ACCENT_RED };
        draw_label_value_colored(scene, inner_x, cursor_y, "Telegram Bot", bot_status, inner_w, bot_color);
    }
}

// --- Drawing primitives ---
// Note: vello 0.7 does not have a built-in text API for Scene.
// We render text as simple geometric glyphs (monospace block characters).
// For a production app, you would integrate a font rasterizer.
// Here we use small rectangles to approximate characters.

const CHAR_W: f64 = 7.0;
const CHAR_H: f64 = 12.0;
const CHAR_GAP: f64 = 1.0;

/// Draw text as simplified block glyphs.
/// Each character is represented by a small filled rectangle pattern.
fn draw_text(scene: &mut Scene, x: f64, y: f64, text: &str, color: Color, size: f64) {
    let scale = size / 14.0;
    let cw = CHAR_W * scale;
    let ch = CHAR_H * scale;
    let gap = CHAR_GAP * scale;

    for (i, ch_byte) in text.bytes().enumerate() {
        let cx = x + i as f64 * (cw + gap);

        if ch_byte == b' ' {
            continue;
        }

        // Render each character as a pattern of small rects that roughly
        // approximates the glyph shape. We use a simple 5x7 bitmap font.
        draw_bitmap_char(scene, cx, y - ch, cw, ch, ch_byte, color);
    }
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

/// Draw centered text.
fn draw_centered_text(scene: &mut Scene, width: f64, height: f64, text: &str, color: Color, size: f64) {
    let scale = size / 14.0;
    let text_width = text.len() as f64 * (CHAR_W * scale + CHAR_GAP * scale);
    let x = (width - text_width) / 2.0;
    let y = height / 2.0;
    draw_text(scene, x, y, text, color, size);
}

/// Draw a filled circle.
fn draw_circle(scene: &mut Scene, cx: f64, cy: f64, r: f64, color: Color) {
    let circle = vello::kurbo::Circle::new(Point::new(cx, cy), r);
    scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &circle);
}

/// Draw a section header bar.
fn draw_section_header(scene: &mut Scene, x: f64, y: f64, w: f64, title: &str) -> f64 {
    let rect = RoundedRect::new(x, y, x + w, y + SECTION_HEIGHT - 4.0, 4.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, SECTION_BG, None, &rect);
    draw_text(scene, x + 8.0, y + SECTION_HEIGHT - 16.0, title, TEXT_SECONDARY, 22.0);
    y + SECTION_HEIGHT
}

/// Draw a label: value pair.
fn draw_label_value(scene: &mut Scene, x: f64, y: f64, label: &str, value: &str, _w: f64) {
    draw_text(scene, x + 4.0, y + 24.0, label, TEXT_SECONDARY, 24.0);
    // Right-align value (scaled for 2x text)
    let value_width = value.len() as f64 * 14.0;
    draw_text(scene, x + _w - value_width - 4.0, y + 24.0, value, TEXT_PRIMARY, 24.0);
}

/// Draw a label: value pair with custom value color.
fn draw_label_value_colored(scene: &mut Scene, x: f64, y: f64, label: &str, value: &str, w: f64, color: Color) {
    draw_text(scene, x + 4.0, y + 24.0, label, TEXT_SECONDARY, 24.0);
    let value_width = value.len() as f64 * 14.0;
    draw_text(scene, x + w - value_width - 4.0, y + 24.0, value, color, 24.0);
}

/// Draw a labeled progress bar.
fn draw_label_bar(scene: &mut Scene, x: f64, y: f64, label: &str, percent: f64, w: f64, bar_color: Color) {
    let label_w = 120.0;  // doubled
    draw_text(scene, x + 4.0, y + 24.0, label, TEXT_SECONDARY, 24.0);

    // Bar background
    let bar_x = x + label_w;
    let bar_w = w - label_w - 80.0;
    let bar_y = y + 8.0;
    let bg_rect = RoundedRect::new(bar_x, bar_y, bar_x + bar_w, bar_y + BAR_HEIGHT, 3.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, BAR_BG, None, &bg_rect);

    // Bar fill
    let fill_w = (bar_w * percent / 100.0).max(0.0).min(bar_w);
    if fill_w > 0.0 {
        let fill_rect = RoundedRect::new(bar_x, bar_y, bar_x + fill_w, bar_y + BAR_HEIGHT, 3.0);
        scene.fill(Fill::NonZero, Affine::IDENTITY, bar_color, None, &fill_rect);
    }

    // Percentage text
    let pct_str = format!("{:.0}%", percent);
    draw_text(scene, x + w - 70.0, y + 24.0, &pct_str, TEXT_PRIMARY, 24.0);
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
fn draw_3d_visualization(
    scene: &mut Scene,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    instance: &crate::protocol::LobsterInstance,
    elapsed: f64,
) {
    let state = &instance.state;

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
    let header_y = draw_section_header(scene, x + PANEL_PADDING, y + PANEL_PADDING, w - 2.0 * PANEL_PADDING, "3D OVERVIEW");

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

    // --- Sessions pillars (left side of the isometric view) ---
    let claude_sessions: Vec<_> = state.sessions.iter()
        .filter(|s| s.name == "claude")
        .collect();
    let session_count = claude_sessions.len();

    // Draw session pillars
    for (i, session) in claude_sessions.iter().enumerate() {
        let grid_x = -80.0 * scale + (i as f64) * 50.0 * scale;
        let grid_z = -40.0 * scale;

        // Height based on memory usage, with a subtle breathing animation
        let base_height = (session.memory_mb / 10.0).clamp(20.0, 120.0) * scale;
        let breath = (elapsed * 1.5 + i as f64 * 0.7).sin() * 3.0 * scale;
        let pillar_h = base_height + breath;
        let pillar_w = 30.0 * scale;

        // Shadow on ground
        draw_iso_shadow(scene, origin_x, origin_y, grid_x, grid_z, pillar_w, scale);

        // Draw the isometric pillar (box)
        draw_iso_box(
            scene, origin_x, origin_y,
            grid_x, 0.0, grid_z,
            pillar_w, pillar_h, pillar_w * 0.8,
            ISO_SESSION_TOP, ISO_SESSION_LEFT, ISO_SESSION_RIGHT,
        );

        // Label below the pillar
        let (lx, ly) = iso_to_screen(grid_x, -12.0 * scale, grid_z);
        draw_text(scene, origin_x + lx - 10.0 * scale, origin_y + ly, &format!("S{}", i + 1), TEXT_PRIMARY, 16.0 * scale);
    }

    // Session count label
    let label_x = viz_x + 8.0;
    let label_y = viz_y + viz_h - 10.0;
    draw_text(scene, label_x, label_y, &format!("{} session{}", session_count, if session_count != 1 { "s" } else { "" }), TEXT_PRIMARY, 18.0);

    // --- Task cubes (right side of the isometric view) ---
    let tasks = &state.tasks.summary;
    let inbox_count = state.message_queues.inbox.count;

    // Stack cubes in a pyramid-ish arrangement
    let cube_size = 22.0 * scale;
    let task_base_x = 30.0 * scale;
    let task_base_z = -30.0 * scale;

    // Draw inbox items as red cubes (incoming tasks)
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
        draw_text(scene, origin_x + lx, origin_y + ly, &format!("+{}", inbox_count - 5), TEXT_PRIMARY, 14.0 * scale);
    }

    // Stack completed tasks as green cubes (solid base)
    let completed = tasks.completed.min(10) as usize;
    draw_task_stack(
        scene, origin_x, origin_y,
        task_base_x, task_base_z,
        cube_size, completed,
        ISO_TASK_DONE_TOP, ISO_TASK_DONE_LEFT, ISO_TASK_DONE_RIGHT,
        elapsed, 0.0, scale,
    );

    // Stack in-progress tasks on top of completed (blue cubes)
    let in_progress = tasks.in_progress.min(5) as usize;
    let completed_height = completed as f64 * cube_size * 0.65;
    draw_task_stack(
        scene, origin_x, origin_y,
        task_base_x + cube_size * 0.3, task_base_z + cube_size * 0.3,
        cube_size * 0.9, in_progress,
        ISO_TASK_ACTIVE_TOP, ISO_TASK_ACTIVE_LEFT, ISO_TASK_ACTIVE_RIGHT,
        elapsed, completed_height, scale,
    );

    // Stack pending tasks as amber cubes (offset slightly)
    let pending = tasks.pending.min(8) as usize;
    draw_task_stack(
        scene, origin_x, origin_y,
        task_base_x + cube_size * 1.8, task_base_z,
        cube_size * 0.85, pending,
        ISO_TASK_PENDING_TOP, ISO_TASK_PENDING_LEFT, ISO_TASK_PENDING_RIGHT,
        elapsed, 0.0, scale,
    );

    // Task legend at bottom-right
    let legend_x = viz_x + viz_w - 180.0;
    let legend_y = viz_y + viz_h - 50.0;
    draw_legend_dot(scene, legend_x, legend_y, ISO_TASK_DONE_TOP);
    draw_text(scene, legend_x + 14.0, legend_y + 4.0, &format!("{} done", tasks.completed), TEXT_PRIMARY, 14.0);
    draw_legend_dot(scene, legend_x, legend_y + 16.0, ISO_TASK_ACTIVE_TOP);
    draw_text(scene, legend_x + 14.0, legend_y + 20.0, &format!("{} active", tasks.in_progress), TEXT_PRIMARY, 14.0);
    draw_legend_dot(scene, legend_x, legend_y + 32.0, ISO_TASK_PENDING_TOP);
    draw_text(scene, legend_x + 14.0, legend_y + 36.0, &format!("{} pending", tasks.pending), TEXT_PRIMARY, 14.0);
    if inbox_count > 0 {
        draw_legend_dot(scene, legend_x, legend_y + 48.0, ISO_INBOX_TOP);
        draw_text(scene, legend_x + 14.0, legend_y + 52.0, &format!("{} inbox", inbox_count), TEXT_PRIMARY, 14.0);
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
