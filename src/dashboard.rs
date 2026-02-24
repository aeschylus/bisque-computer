//! Dashboard rendering using vello.
//!
//! Renders Lobster instance data as a visual dashboard using typography-first
//! design principles. No boxes, no panels, no progress bars, no colored badges.
//! Hierarchy is communicated through font size, opacity, and whitespace.
//!
//! Visual theme: bisque beige background, black text at varying opacities.
//! Fonts: Optima for readable text, Monaco for monospace (font stacks with fallbacks).
//! On app open: Ulysses splash quote fades in and out.

use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill, FontData};
use vello::{Glyph, Scene};

use crate::design::DesignTokens;
use crate::protocol::{ConnectionStatus, LobsterInstance};
use crate::voice::VoiceUiState;
use crate::ws_client::SharedInstances;

// --- Color palette (typography-only: background + black at varying opacities) ---

const BG_COLOR: Color = Color::new([1.0, 0.894, 0.769, 1.0]);        // CSS bisque background
const TEXT_PRIMARY: Color = Color::new([0.0, 0.0, 0.0, 1.0]);        // pure black (#000000)
const TEXT_SECONDARY: Color = Color::new([0.0, 0.0, 0.0, 0.85]);     // black at 85% opacity
const TEXT_SECTION: Color = Color::new([0.0, 0.0, 0.0, 0.92]);       // black at 92% opacity (section titles)
const TEXT_ANNOTATION: Color = Color::new([0.0, 0.0, 0.0, 0.75]);    // black at 75% opacity (small annotations)
const RULE_COLOR: Color = Color::new([0.0, 0.0, 0.0, 0.15]);         // black at 15% opacity (thin rules)

// --- Typography design system ---

const TITLE_SIZE: f64 = 44.0;       // Page title
const SECTION_SIZE: f64 = 18.0;     // Section titles
const DATA_PRIMARY_SIZE: f64 = 24.0; // Primary data
const DATA_SECONDARY_SIZE: f64 = 20.0; // Labels, secondary data
const ANNOTATION_SIZE: f64 = 16.0;  // Small annotations

// --- Layout constants ---

const LEFT_MARGIN: f64 = 48.0;
const SECTION_SPACING: f64 = 32.0;
const LINE_HEIGHT_FACTOR: f64 = 1.4;
const RULE_THICKNESS: f64 = 0.5;

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
    tokens: &DesignTokens,
) {
    // Background fill - bisque beige (from design tokens)
    let bg_rect = Rect::new(0.0, 0.0, width, height);
    scene.fill(Fill::NonZero, Affine::IDENTITY, tokens.bg_color(), None, &bg_rect);

    // Scale factor: allows Cmd+=/- to resize all dashboard text.
    let scale = tokens.type_scale.base / 18.0;
    let title_size = TITLE_SIZE * scale;
    let section_size = SECTION_SIZE * scale;
    let data_primary_size = DATA_PRIMARY_SIZE * scale;
    let data_secondary_size = DATA_SECONDARY_SIZE * scale;
    let annotation_size = ANNOTATION_SIZE * scale;
    // Suppress unused warnings — not all may be used depending on data.
    let _ = (title_size, section_size, data_primary_size, data_secondary_size, annotation_size);

    // Ulysses splash quote overlay (fades in and out on app open)
    if elapsed < TOTAL_SPLASH_DURATION {
        draw_splash_quote(scene, width, height, elapsed, font_data);
    }

    let instances = instances.lock().unwrap();

    if instances.is_empty() {
        draw_centered_text(scene, width, height, "No Lobster instances configured", TEXT_SECONDARY, 40.0 * scale, font_data);
        return;
    }

    // Page title — large Optima text with generous whitespace
    let title_y = 56.0;
    draw_text_with_font(scene, LEFT_MARGIN, title_y, "Lobster Dashboard", TEXT_PRIMARY, title_size, font_data);

    // Connection status as secondary text next to title
    let connected = instances
        .iter()
        .filter(|i| i.status == ConnectionStatus::Connected)
        .count();
    let status_text = format!("{}/{} connected", connected, instances.len());
    draw_text_with_font(scene, width - 300.0, title_y, &status_text, TEXT_SECONDARY, data_secondary_size, font_data);

    // Voice input hint (shown when enabled)
    if voice_enabled {
        let hint = match voice_ui {
            VoiceUiState::Idle => "Shift+Enter: voice",
            VoiceUiState::Recording => "Recording...",
            VoiceUiState::Transcribing => "Transcribing...",
            VoiceUiState::Done(_) => "Sent",
            VoiceUiState::Error(_) => "Voice error",
        };
        let hint_opacity = match voice_ui {
            VoiceUiState::Recording => 1.0_f32,
            VoiceUiState::Transcribing => 0.85,
            _ => 0.75,
        };
        let hint_color = Color::new([0.0_f32, 0.0, 0.0, hint_opacity]);
        draw_text_with_font(scene, width - 300.0, title_y + 28.0, hint, hint_color, annotation_size, font_data);
    }

    // Content area starts below the title
    let content_top = title_y + 32.0;
    let content_width = width - LEFT_MARGIN * 2.0;
    let content_height = height - content_top - LEFT_MARGIN;

    let num_instances = instances.len();

    if num_instances == 1 {
        // Single instance: full width
        draw_instance_panel(scene, LEFT_MARGIN, content_top, content_width, content_height, &instances[0], font_data, scale);
    } else {
        // Multiple instances: vertical flow or two-column
        let cols = if num_instances <= 2 { num_instances } else if num_instances <= 4 { 2 } else { 3 };
        let rows = (num_instances + cols - 1) / cols;
        let col_gap = 48.0;
        let row_gap = SECTION_SPACING;

        let panel_width = (content_width - (cols as f64 - 1.0) * col_gap) / cols as f64;
        let panel_height = (content_height - (rows as f64 - 1.0) * row_gap) / rows as f64;

        for (idx, instance) in instances.iter().enumerate() {
            let col = idx % cols;
            let row = idx / cols;
            let x = LEFT_MARGIN + col as f64 * (panel_width + col_gap);
            let y = content_top + row as f64 * (panel_height + row_gap);

            draw_instance_panel(scene, x, y, panel_width, panel_height, instance, font_data, scale);
        }
    }

    // Voice input overlay — drawn on top of everything else
    drop(instances);
    draw_voice_indicator(scene, width, height, voice_ui, font_data);
}

/// Draw the voice recording / transcribing / result overlay.
///
/// Pure typography: just text in the bottom-right corner, no badges or pills.
fn draw_voice_indicator(
    scene: &mut Scene,
    width: f64,
    height: f64,
    voice_ui: &VoiceUiState,
    font_data: Option<&FontData>,
) {
    let right_margin = LEFT_MARGIN;
    let bottom_margin = LEFT_MARGIN;

    match voice_ui {
        VoiceUiState::Idle => {} // Nothing to draw

        VoiceUiState::Recording => {
            draw_text_with_font(
                scene,
                width - right_margin - 180.0,
                height - bottom_margin,
                "Recording...",
                TEXT_PRIMARY,
                DATA_PRIMARY_SIZE,
                font_data,
            );
        }

        VoiceUiState::Transcribing => {
            draw_text_with_font(
                scene,
                width - right_margin - 200.0,
                height - bottom_margin,
                "Transcribing...",
                TEXT_SECONDARY,
                DATA_SECONDARY_SIZE,
                font_data,
            );
        }

        VoiceUiState::Done(text) => {
            // Word-wrapped transcribed text in the bottom-right area
            let font_size = DATA_SECONDARY_SIZE;
            let char_w = 11.0;
            let max_w = width * 0.5;
            let chars_per_line = (max_w / char_w).max(10.0) as usize;
            let line_height = font_size * LINE_HEIGHT_FACTOR;

            let full_text = format!("Sent: {}", text);
            let mut lines: Vec<String> = Vec::new();
            let mut current_line = String::new();
            for word in full_text.split_whitespace() {
                if current_line.is_empty() {
                    current_line = word.to_string();
                } else if current_line.len() + 1 + word.len() <= chars_per_line {
                    current_line.push(' ');
                    current_line.push_str(word);
                } else {
                    lines.push(current_line);
                    current_line = word.to_string();
                }
            }
            if !current_line.is_empty() {
                lines.push(current_line);
            }
            if lines.is_empty() {
                lines.push(full_text);
            }

            let num_lines = lines.len();
            let block_height = num_lines as f64 * line_height;
            let start_y = height - bottom_margin - block_height + line_height;

            for (i, line) in lines.iter().enumerate() {
                draw_text_with_font(
                    scene,
                    width - right_margin - max_w,
                    start_y + i as f64 * line_height,
                    line,
                    TEXT_SECONDARY,
                    font_size,
                    font_data,
                );
            }
        }

        VoiceUiState::Error(msg) => {
            let max_chars = 50;
            let display_msg: String = if msg.len() > max_chars {
                format!("{}...", &msg[..max_chars])
            } else {
                msg.clone()
            };
            let label = format!("Voice error: {}", display_msg);
            draw_text_with_font(
                scene,
                width - right_margin - 500.0,
                height - bottom_margin,
                &label,
                TEXT_SECONDARY,
                DATA_SECONDARY_SIZE,
                font_data,
            );
        }
    }
}

/// Draw a single Lobster instance using typography-first layout.
///
/// No panels, no borders. Just text with hierarchy created through
/// font size, opacity, and whitespace.
fn draw_instance_panel(
    scene: &mut Scene,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    instance: &LobsterInstance,
    font_data: Option<&FontData>,
    scale: f64,
) {
    let data_primary_size = DATA_PRIMARY_SIZE * scale;
    let data_secondary_size = DATA_SECONDARY_SIZE * scale;
    let section_size = SECTION_SIZE * scale;
    let annotation_size = ANNOTATION_SIZE * scale;
    let _ = (section_size, annotation_size);
    let mut cursor_y = y;

    // --- Hostname in large text ---
    let hostname = if instance.status == ConnectionStatus::Connected {
        &instance.state.system.hostname
    } else {
        &instance.url
    };
    cursor_y += 36.0;
    draw_text_with_font(scene, x, cursor_y, hostname, TEXT_PRIMARY, 36.0, font_data);

    // Connection status as small secondary text below
    let status_str = match &instance.status {
        ConnectionStatus::Connected => "connected".to_string(),
        ConnectionStatus::Connecting => "connecting...".to_string(),
        ConnectionStatus::Disconnected => "disconnected".to_string(),
        ConnectionStatus::Error(e) => format!("error: {}", &e[..e.len().min(30)]),
    };
    cursor_y += data_secondary_size * LINE_HEIGHT_FACTOR;
    draw_text_with_font(scene, x, cursor_y, &status_str, TEXT_SECONDARY, data_secondary_size, font_data);

    if instance.status != ConnectionStatus::Connected {
        return;
    }

    let state = &instance.state;
    cursor_y += SECTION_SPACING;

    // --- System section ---
    cursor_y = draw_section_header_with_font(scene, x, cursor_y, w, "System", font_data, scale);

    // Uptime
    let uptime_str = format_uptime(state.system.uptime_seconds);
    draw_label_value_with_font(scene, x, cursor_y, "Uptime", &uptime_str, w, font_data, scale);
    cursor_y += data_primary_size * LINE_HEIGHT_FACTOR;

    // CPU, Memory, Disk as text-only: "CPU  42%"
    let cpu_str = format!("{:.0}%", state.system.cpu.percent);
    draw_label_value_with_font(scene, x, cursor_y, "CPU", &cpu_str, w, font_data, scale);
    cursor_y += data_primary_size * LINE_HEIGHT_FACTOR;

    let mem_str = format!("{:.0}%", state.system.memory.percent);
    draw_label_value_with_font(scene, x, cursor_y, "Memory", &mem_str, w, font_data, scale);
    cursor_y += data_primary_size * LINE_HEIGHT_FACTOR;

    let disk_str = format!("{:.0}%", state.system.disk.percent);
    draw_label_value_with_font(scene, x, cursor_y, "Disk", &disk_str, w, font_data, scale);
    cursor_y += SECTION_SPACING;

    // --- Sessions section ---
    cursor_y = draw_section_header_with_font(scene, x, cursor_y, w, "Sessions", font_data, scale);

    let claude_sessions: Vec<_> = state.sessions.iter()
        .filter(|s| s.name == "claude")
        .collect();
    let session_count = claude_sessions.len();
    draw_label_value_with_font(scene, x, cursor_y, "Active", &format!("{}", session_count), w, font_data, scale);
    cursor_y += data_primary_size * LINE_HEIGHT_FACTOR;

    // Show each session as text
    for session in claude_sessions.iter().take(3) {
        let label = format!("PID {}", session.pid);
        let value = format!("{:.0} MB", session.memory_mb);
        draw_text_with_font(scene, x + 16.0, cursor_y, &label, TEXT_ANNOTATION, data_secondary_size, font_data);
        let value_width = value.len() as f64 * 12.0;
        draw_text_with_font(scene, (x + w - value_width).max(x + 100.0), cursor_y, &value, TEXT_PRIMARY, data_secondary_size, font_data);
        cursor_y += data_secondary_size * LINE_HEIGHT_FACTOR;
    }
    if session_count > 3 {
        draw_text_with_font(scene, x + 16.0, cursor_y, &format!("+{} more", session_count - 3), TEXT_ANNOTATION, annotation_size, font_data);
        cursor_y += annotation_size * LINE_HEIGHT_FACTOR;
    }
    cursor_y += SECTION_SPACING - data_secondary_size * LINE_HEIGHT_FACTOR;

    // --- Messages section ---
    if cursor_y + 100.0 < y + h {
        cursor_y = draw_section_header_with_font(scene, x, cursor_y, w, "Messages", font_data, scale);

        let queues = &state.message_queues;
        let queue_items = [
            ("Inbox", queues.inbox.count),
            ("Processed", queues.processed.count),
            ("Sent", queues.sent.count),
            ("Failed", queues.failed.count),
        ];

        for (label, count) in &queue_items {
            draw_label_value_with_font(scene, x, cursor_y, label, &count.to_string(), w, font_data, scale);
            cursor_y += data_primary_size * LINE_HEIGHT_FACTOR;
        }
        cursor_y += SECTION_SPACING - data_primary_size * LINE_HEIGHT_FACTOR;
    }

    // --- Activity section ---
    if cursor_y + 80.0 < y + h {
        cursor_y = draw_section_header_with_font(scene, x, cursor_y, w, "Activity (24h)", font_data, scale);

        let activity = &state.conversation_activity;
        draw_label_value_with_font(scene, x, cursor_y, "Received", &activity.messages_received_24h.to_string(), w, font_data, scale);
        cursor_y += data_primary_size * LINE_HEIGHT_FACTOR;
        draw_label_value_with_font(scene, x, cursor_y, "Replied", &activity.replies_sent_24h.to_string(), w, font_data, scale);
        cursor_y += SECTION_SPACING;
    }

    // --- Health section ---
    if cursor_y + 80.0 < y + h {
        cursor_y = draw_section_header_with_font(scene, x, cursor_y, w, "Health", font_data, scale);

        let health = &state.health;
        let hb_status = match health.heartbeat_age_seconds {
            Some(age) if age < 300 => format!("{}s ago", age),
            Some(age) => format!("stale ({}s)", age),
            None => "unknown".to_string(),
        };
        draw_label_value_with_font(scene, x, cursor_y, "Heartbeat", &hb_status, w, font_data, scale);
        cursor_y += data_primary_size * LINE_HEIGHT_FACTOR;

        let bot_status = if health.telegram_bot_running { "running" } else { "stopped" };
        draw_label_value_with_font(scene, x, cursor_y, "Telegram Bot", bot_status, w, font_data, scale);
        cursor_y += SECTION_SPACING;
    }

    // --- Agents section ---
    if cursor_y + 60.0 < y + h {
        cursor_y = draw_section_header_with_font(scene, x, cursor_y, w, "Agents", font_data, scale);

        let agents = &state.subagent_list.agents;
        if agents.is_empty() {
            draw_text_with_font(scene, x, cursor_y, "No agents running", TEXT_ANNOTATION, data_secondary_size, font_data);
            cursor_y += data_secondary_size * LINE_HEIGHT_FACTOR;
        } else {
            for agent in agents.iter().take(4) {
                if cursor_y + data_secondary_size * LINE_HEIGHT_FACTOR > y + h {
                    break;
                }
                // Description
                let desc = if agent.description.len() > 52 {
                    format!("{}...", &agent.description[..52])
                } else {
                    agent.description.clone()
                };
                draw_text_with_font(scene, x, cursor_y, &desc, TEXT_PRIMARY, data_secondary_size, font_data);

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
                let stats_w = stats_str.len() as f64 * 10.0;
                draw_text_with_font(scene, (x + w - stats_w).max(x + 20.0), cursor_y, &stats_str, TEXT_ANNOTATION, annotation_size, font_data);
                cursor_y += data_secondary_size * LINE_HEIGHT_FACTOR;
            }
            if agents.len() > 4 {
                draw_text_with_font(scene, x, cursor_y, &format!("+{} more", agents.len() - 4), TEXT_ANNOTATION, annotation_size, font_data);
                cursor_y += annotation_size * LINE_HEIGHT_FACTOR;
            }
        }
        cursor_y += SECTION_SPACING - data_secondary_size * LINE_HEIGHT_FACTOR;
    }

    // --- Memory section ---
    if cursor_y + 60.0 < y + h {
        cursor_y = draw_section_header_with_font(scene, x, cursor_y, w, "Memory", font_data, scale);

        let mem = &state.memory;
        draw_label_value_with_font(scene, x, cursor_y, "Events", &mem.total_events.to_string(), w, font_data, scale);
        cursor_y += data_primary_size * LINE_HEIGHT_FACTOR;

        // Projects list
        if !mem.projects.is_empty() && cursor_y + data_secondary_size * LINE_HEIGHT_FACTOR < y + h {
            let projects_str = if mem.projects.len() > 4 {
                format!("{} (+{})", mem.projects[..4].join(", "), mem.projects.len() - 4)
            } else {
                mem.projects.join(", ")
            };
            draw_label_value_with_font(scene, x, cursor_y, "Projects", &projects_str, w, font_data, scale);
            cursor_y += data_primary_size * LINE_HEIGHT_FACTOR;
        }

        // Recent events
        for event in mem.recent_events.iter().take(4) {
            if cursor_y + data_secondary_size * LINE_HEIGHT_FACTOR > y + h {
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
            draw_text_with_font(scene, x, cursor_y, &line, TEXT_ANNOTATION, data_secondary_size, font_data);
            cursor_y += data_secondary_size * LINE_HEIGHT_FACTOR;
        }

        // Last consolidation
        if cursor_y + data_secondary_size * LINE_HEIGHT_FACTOR < y + h {
            let consol_str = match &mem.consolidations.last_consolidation_at {
                Some(ts) => {
                    let short = if ts.len() > 16 { &ts[..16] } else { ts.as_str() };
                    format!("Last consolidation: {}", short)
                }
                None => "Last consolidation: never".to_string(),
            };
            draw_text_with_font(scene, x, cursor_y, &consol_str, TEXT_ANNOTATION, annotation_size, font_data);
        }
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
#[allow(dead_code)]
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

/// Draw a section header: section title text with a thin line underneath.
///
/// Returns the y position below the header where content should start.
fn draw_section_header_with_font(scene: &mut Scene, x: f64, y: f64, w: f64, title: &str, font_data: Option<&FontData>, scale: f64) -> f64 {
    let section_size = SECTION_SIZE * scale;
    // Section title text
    draw_text_with_font(scene, x, y, title, TEXT_SECTION, section_size, font_data);

    // Thin horizontal rule underneath
    let rule_y = y + 6.0;
    let rule_rect = Rect::new(x, rule_y, x + w, rule_y + RULE_THICKNESS);
    scene.fill(Fill::NonZero, Affine::IDENTITY, RULE_COLOR, None, &rule_rect);

    // Return the y position for content below
    y + section_size * LINE_HEIGHT_FACTOR + 4.0
}

/// Draw a label: value pair. Label in secondary weight, value right-aligned in primary weight.
fn draw_label_value_with_font(scene: &mut Scene, x: f64, y: f64, label: &str, value: &str, w: f64, font_data: Option<&FontData>, scale: f64) {
    let data_secondary_size = DATA_SECONDARY_SIZE * scale;
    let data_primary_size = DATA_PRIMARY_SIZE * scale;
    draw_text_with_font(scene, x, y, label, TEXT_SECONDARY, data_secondary_size, font_data);
    let value_width = value.len() as f64 * (12.0 * scale);
    draw_text_with_font(scene, (x + w - value_width).max(x + 100.0), y, value, TEXT_PRIMARY, data_primary_size, font_data);
}

// --- Splash quote rendering ---

/// Compute the splash quote alpha based on elapsed time
fn splash_alpha(elapsed: f64) -> f32 {
    if elapsed >= TOTAL_SPLASH_DURATION {
        return 0.0;
    }
    if elapsed < FADE_IN_DURATION {
        let t = (elapsed / FADE_IN_DURATION) as f32;
        ease_in_out(t)
    } else if elapsed < FADE_IN_DURATION + HOLD_DURATION {
        1.0
    } else {
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
        let font_size = 32.0;
        let max_chars_per_line = ((width * 0.7) / (font_size * 0.6)) as usize;
        let start_x = width * 0.15;
        let mut y = height * 0.20;
        let line_height_px = font_size * 1.5;

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

/// Specification for a selectable text region (text, font size, origin, max width).
pub struct SelectableRegionSpec {
    pub text: String,
    pub font_size: f32,
    pub origin: (f64, f64),
    pub max_width: Option<f32>,
}

/// Compute selectable text regions for all instance panels.
///
/// This replicates the layout math from `draw_instance_panel` so that
/// `SelectableText` regions align with the rendered text.
pub fn compute_selectable_regions(
    instances: &[LobsterInstance],
    width: f64,
    _height: f64,
) -> Vec<SelectableRegionSpec> {
    let mut regions = Vec::new();

    if instances.is_empty() {
        return regions;
    }

    let title_y = 56.0;
    let content_top = title_y + 32.0;
    let content_width = width - LEFT_MARGIN * 2.0;
    let num_instances = instances.len();

    // Compute panel positions (same logic as render_dashboard).
    let panel_positions: Vec<(f64, f64, f64, f64)> = if num_instances == 1 {
        vec![(LEFT_MARGIN, content_top, content_width, 9999.0)]
    } else {
        let cols = if num_instances <= 2 { num_instances } else if num_instances <= 4 { 2 } else { 3 };
        let col_gap = 48.0;
        let panel_width = (content_width - (cols as f64 - 1.0) * col_gap) / cols as f64;

        instances.iter().enumerate().map(|(idx, _)| {
            let col = idx % cols;
            let row = idx / cols;
            let x = LEFT_MARGIN + col as f64 * (panel_width + col_gap);
            let y = content_top + row as f64 * 600.0; // approximate
            (x, y, panel_width, 600.0)
        }).collect()
    };

    for (idx, instance) in instances.iter().enumerate() {
        let (x, y, w, _h) = panel_positions[idx];
        let max_w = Some(w as f32);
        let mut cursor_y = y;

        // Hostname
        cursor_y += 36.0;
        let hostname = if instance.status == ConnectionStatus::Connected {
            instance.state.system.hostname.clone()
        } else {
            instance.url.clone()
        };
        let hostname_font = 36.0_f32;
        regions.push(SelectableRegionSpec {
            text: hostname,
            font_size: hostname_font,
            origin: (x, cursor_y - hostname_font as f64 * 0.8),
            max_width: max_w,
        });

        // Status text
        let status_str = match &instance.status {
            ConnectionStatus::Connected => "connected".to_string(),
            ConnectionStatus::Connecting => "connecting...".to_string(),
            ConnectionStatus::Disconnected => "disconnected".to_string(),
            ConnectionStatus::Error(e) => format!("error: {}", &e[..e.len().min(30)]),
        };
        cursor_y += DATA_SECONDARY_SIZE as f64 * LINE_HEIGHT_FACTOR;
        let status_font = DATA_SECONDARY_SIZE as f32;
        regions.push(SelectableRegionSpec {
            text: status_str,
            font_size: status_font,
            origin: (x, cursor_y - status_font as f64 * 0.8),
            max_width: max_w,
        });

        if instance.status != ConnectionStatus::Connected {
            continue;
        }

        let state = &instance.state;

        // Skip System section header + 4 data lines
        cursor_y += SECTION_SPACING;
        cursor_y += SECTION_SIZE * LINE_HEIGHT_FACTOR + 4.0; // section header
        cursor_y += DATA_PRIMARY_SIZE * LINE_HEIGHT_FACTOR * 4.0; // uptime, cpu, mem, disk
        cursor_y += SECTION_SPACING;

        // Sessions section
        cursor_y += SECTION_SIZE * LINE_HEIGHT_FACTOR + 4.0; // header
        cursor_y += DATA_PRIMARY_SIZE * LINE_HEIGHT_FACTOR; // "Active: N"

        let claude_sessions: Vec<_> = state.sessions.iter()
            .filter(|s| s.name == "claude")
            .collect();
        let session_count = claude_sessions.len();

        for session in claude_sessions.iter().take(3) {
            let label = format!("PID {} -- {:.0} MB", session.pid, session.memory_mb);
            let session_font = DATA_SECONDARY_SIZE as f32;
            regions.push(SelectableRegionSpec {
                text: label,
                font_size: session_font,
                origin: (x + 16.0, cursor_y - session_font as f64 * 0.8),
                max_width: max_w,
            });
            cursor_y += DATA_SECONDARY_SIZE * LINE_HEIGHT_FACTOR;
        }
        if session_count > 3 {
            cursor_y += ANNOTATION_SIZE * LINE_HEIGHT_FACTOR;
        }

        // Skip Messages, Activity, Health sections (decorative stats)
        // ... agents are the interesting selectable content

        // Approximate skip to agents section
        cursor_y += SECTION_SPACING * 4.0 + DATA_PRIMARY_SIZE * LINE_HEIGHT_FACTOR * 8.0;

        // AGENTS section
        let agents = &state.subagent_list.agents;
        if !agents.is_empty() {
            cursor_y += SECTION_SIZE * LINE_HEIGHT_FACTOR + 4.0; // header
            for agent in agents.iter().take(4) {
                let desc = if agent.description.len() > 52 {
                    format!("{}...", &agent.description[..52])
                } else {
                    agent.description.clone()
                };
                let agent_font = DATA_SECONDARY_SIZE as f32;
                regions.push(SelectableRegionSpec {
                    text: desc,
                    font_size: agent_font,
                    origin: (x, cursor_y - agent_font as f64 * 0.8),
                    max_width: max_w,
                });
                cursor_y += DATA_SECONDARY_SIZE * LINE_HEIGHT_FACTOR;
            }
        }
    }

    regions
}

/// Render the first-run setup screen.
///
/// Shown when `~/.config/bisque-computer/server` is absent. The user types a
/// WebSocket URL then presses Enter to save and connect.
/// Purely typographic: no input field box, just text and a cursor line.
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

    // Title
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
    draw_text_with_font(scene, cx - hint_w / 2.0, cy - 28.0, hint, TEXT_ANNOTATION, hint_size, font_data);

    // Typed text (no box, just text with cursor)
    let field_y = cy + 24.0;
    let (display_text, text_color) = if input_buffer.is_empty() {
        ("ws://".to_string(), TEXT_SECONDARY)
    } else {
        (input_buffer.to_string(), TEXT_PRIMARY)
    };
    let text_size = 28.0_f64;
    let text_w = display_text.len() as f64 * text_size * 0.55;
    let text_x = cx - text_w / 2.0;
    draw_text_with_font(scene, text_x, field_y, &display_text, text_color, text_size, font_data);

    // Thin cursor line after the text
    let char_advance = 16.0_f64;
    let cursor_x = if input_buffer.is_empty() {
        text_x + 5.0 * char_advance // after "ws://"
    } else {
        text_x + input_buffer.len() as f64 * char_advance
    };
    let cursor_rect = Rect::new(cursor_x, field_y - 20.0, cursor_x + 1.5, field_y + 4.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, TEXT_PRIMARY, None, &cursor_rect);

    // Footer
    let footer = "Press Enter to connect  |  Press Escape to quit";
    let footer_size = 22.0_f64;
    let footer_w = footer.len() as f64 * footer_size * 0.55;
    draw_text_with_font(scene, cx - footer_w / 2.0, cy + 120.0, footer, TEXT_ANNOTATION, footer_size, font_data);
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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use crate::design::DesignTokens;

    // --- Color constant tests ---

    #[test]
    fn test_bg_color_is_bisque() {
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
    fn test_text_secondary_is_black_with_opacity() {
        let components = TEXT_SECONDARY.components;
        assert_eq!(components[0], 0.0, "TEXT_SECONDARY red should be 0.0");
        assert_eq!(components[1], 0.0, "TEXT_SECONDARY green should be 0.0");
        assert_eq!(components[2], 0.0, "TEXT_SECONDARY blue should be 0.0");
        assert!((components[3] - 0.85).abs() < 0.01, "TEXT_SECONDARY alpha should be ~0.85");
    }

    // --- Splash animation tests ---

    #[test]
    fn test_splash_alpha_starts_at_zero() {
        assert!((splash_alpha(0.0)).abs() < 0.01, "Alpha at t=0 should be ~0");
    }

    #[test]
    fn test_splash_alpha_reaches_full() {
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
        let result = load_readable_font();
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
        assert!(LEFT_MARGIN > 0.0);
        assert!(SECTION_SPACING > 0.0);
        assert!(RULE_THICKNESS > 0.0);
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

    // --- Bitmap font tests ---

    #[test]
    fn test_char_bitmap_all_printable_ascii() {
        for ch in 32u8..=126 {
            let bitmap = get_char_bitmap(ch);
            assert_eq!(bitmap.len(), 7, "Bitmap for '{}' should have 7 rows", ch as char);
        }
    }

    #[test]
    fn test_char_bitmap_letters_not_empty() {
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
        let bitmap = get_char_bitmap(0xFF);
        let expected_box: [u8; 7] = [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111];
        assert_eq!(bitmap, expected_box, "Unknown chars should render as a box");
    }

    // --- Render pipeline integration tests ---

    #[test]
    fn test_render_dashboard_no_instances() {
        let mut scene = Scene::new();
        let instances = Arc::new(Mutex::new(Vec::new()));
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 0.0, None, &VoiceUiState::Idle, false, &DesignTokens::default());
    }

    #[test]
    fn test_render_dashboard_single_disconnected_instance() {
        let mut scene = Scene::new();
        let instance = crate::protocol::LobsterInstance::new("ws://localhost:9100".to_string());
        let instances = Arc::new(Mutex::new(vec![instance]));
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 0.0, None, &VoiceUiState::Idle, false, &DesignTokens::default());
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
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 0.0, None, &VoiceUiState::Idle, false, &DesignTokens::default());
    }

    #[test]
    fn test_render_dashboard_multiple_instances() {
        let mut scene = Scene::new();
        let mut inst1 = crate::protocol::LobsterInstance::new("ws://host1:9100".to_string());
        inst1.status = crate::protocol::ConnectionStatus::Connected;
        inst1.state.system.hostname = "host1".to_string();
        let inst2 = crate::protocol::LobsterInstance::new("ws://host2:9100".to_string());
        let instances = Arc::new(Mutex::new(vec![inst1, inst2]));
        render_dashboard(&mut scene, 1920.0, 1080.0, &instances, 5.0, None, &VoiceUiState::Idle, false, &DesignTokens::default());
    }

    #[test]
    fn test_render_dashboard_during_splash() {
        let mut scene = Scene::new();
        let instances = Arc::new(Mutex::new(Vec::new()));
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 1.0, None, &VoiceUiState::Idle, false, &DesignTokens::default());
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 4.0, None, &VoiceUiState::Idle, false, &DesignTokens::default());
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 7.0, None, &VoiceUiState::Idle, false, &DesignTokens::default());
    }

    #[test]
    fn test_render_dashboard_after_splash() {
        let mut scene = Scene::new();
        let instances = Arc::new(Mutex::new(Vec::new()));
        render_dashboard(&mut scene, 1280.0, 800.0, &instances, 100.0, None, &VoiceUiState::Idle, false, &DesignTokens::default());
    }

    // --- Ulysses quote test ---

    #[test]
    fn test_ulysses_quote_is_nonempty() {
        assert!(!ULYSSES_QUOTE.is_empty(), "Ulysses quote should not be empty");
        assert!(ULYSSES_QUOTE.contains("yes"), "Quote should contain 'yes'");
        assert!(ULYSSES_QUOTE.ends_with("Yes."), "Quote should end with 'Yes.'");
    }
}
