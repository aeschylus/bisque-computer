//! Info/Memory screen — the center screen in the 3-panel layout.
//!
//! Shows recent memory events and active subagents pulled from
//! the live WebSocket state (`DashboardState`).
//!
//! Visual theme matches the dashboard: bisque beige background, black text,
//! doubled text sizes.

use vello::Scene;
use vello::kurbo::{Affine, Rect, RoundedRect};
use vello::peniko::{Color, Fill, FontData};

use crate::design::DesignTokens;
use crate::ws_client::SharedInstances;

// --- Color palette (reused from dashboard theme) ---
const BG_COLOR: Color = Color::new([1.0, 0.894, 0.769, 1.0]);
const PANEL_BG: Color = Color::new([1.0, 0.922, 0.827, 1.0]);
const PANEL_BORDER: Color = Color::new([0.87, 0.72, 0.53, 1.0]);
const HEADER_BG: Color = Color::new([0.80, 0.62, 0.40, 1.0]);
const TEXT_PRIMARY: Color = Color::new([0.0, 0.0, 0.0, 1.0]);
const TEXT_SECONDARY: Color = Color::new([0.0, 0.0, 0.0, 0.65]);
const TEXT_LIGHT: Color = Color::new([0.0, 0.0, 0.0, 1.0]);
const ACCENT_BLUE: Color = Color::new([0.22, 0.46, 0.72, 1.0]);
const ACCENT_GREEN: Color = Color::new([0.20, 0.65, 0.32, 1.0]);
const ACCENT_AMBER: Color = Color::new([0.85, 0.60, 0.10, 1.0]);

const MARGIN: f64 = 24.0;
const PANEL_GAP: f64 = 16.0;
const HEADER_HEIGHT: f64 = 72.0;
const CORNER_RADIUS: f64 = 8.0;
const LINE_HEIGHT: f64 = 32.0;

/// Render the Info screen into the provided scene.
///
/// Displays recent memory events (from `DashboardState.memory.recent_events`)
/// and active subagents (from `DashboardState.subagent_list.agents`).
pub fn render_info_screen(
    scene: &mut Scene,
    width: f64,
    height: f64,
    instances: &SharedInstances,
    font_data: Option<&FontData>,
    tokens: &DesignTokens,
) {
    // Background fill (from design tokens).
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        tokens.bg_color(),
        None,
        &Rect::new(0.0, 0.0, width, height),
    );

    // Header bar.
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        HEADER_BG,
        None,
        &Rect::new(0.0, 0.0, width, HEADER_HEIGHT),
    );
    crate::dashboard::draw_text_pub(
        scene, MARGIN, 50.0, "MEMORY & AGENTS", TEXT_LIGHT, 44.0, font_data,
    );

    let content_top = HEADER_HEIGHT + MARGIN;
    let content_height = height - content_top - MARGIN;
    let half_w = (width - 2.0 * MARGIN - PANEL_GAP) / 2.0;

    // Get the first connected instance's state (or default).
    let instance_state = {
        match instances.lock() {
            Ok(guard) => guard
                .iter()
                .find(|i| i.status == crate::protocol::ConnectionStatus::Connected)
                .map(|i| i.state.clone()),
            Err(_) => None,
        }
    };

    // Left panel: Recent Memory Events.
    draw_memory_panel(
        scene,
        MARGIN,
        content_top,
        half_w,
        content_height,
        instance_state.as_ref(),
        font_data,
    );

    // Right panel: Active Subagents.
    draw_agents_panel(
        scene,
        MARGIN + half_w + PANEL_GAP,
        content_top,
        half_w,
        content_height,
        instance_state.as_ref(),
        font_data,
    );
}

/// Draw the Recent Memory Events panel.
fn draw_memory_panel(
    scene: &mut Scene,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    state: Option<&crate::protocol::DashboardState>,
    font_data: Option<&FontData>,
) {
    // Panel background.
    let panel_rect = RoundedRect::new(x, y, x + w, y + h, CORNER_RADIUS);
    scene.fill(Fill::NonZero, Affine::IDENTITY, PANEL_BG, None, &panel_rect);

    // Panel border.
    use vello::kurbo::Stroke;
    scene.stroke(
        &Stroke::new(1.5),
        Affine::IDENTITY,
        PANEL_BORDER,
        None,
        &panel_rect,
    );

    // Section header.
    let header_rect = RoundedRect::new(x, y, x + w, y + 44.0, CORNER_RADIUS);
    scene.fill(Fill::NonZero, Affine::IDENTITY, ACCENT_BLUE, None, &header_rect);
    crate::dashboard::draw_text_pub(
        scene,
        x + 12.0,
        y + 30.0,
        "RECENT MEMORY",
        Color::new([1.0, 1.0, 1.0, 1.0]),
        22.0,
        font_data,
    );

    let events = state
        .map(|s| s.memory.recent_events.as_slice())
        .unwrap_or(&[]);

    if events.is_empty() {
        crate::dashboard::draw_text_pub(
            scene,
            x + 12.0,
            y + 80.0,
            "No recent memory events",
            TEXT_SECONDARY,
            22.0,
            font_data,
        );
        return;
    }

    let max_visible = ((h - 60.0) / LINE_HEIGHT) as usize;
    let events_to_show = events.iter().rev().take(max_visible);

    let mut row_y = y + 52.0;
    for event in events_to_show {
        let type_label = &event.event_type;
        let content_truncated: String = event.content.chars().take(60).collect();
        let time_short: String = event.timestamp.chars().take(19).collect();

        // Event type badge color.
        let badge_color = match type_label.as_str() {
            "decision" => ACCENT_AMBER,
            "task" => ACCENT_GREEN,
            "link" => ACCENT_BLUE,
            _ => Color::new([0.60, 0.60, 0.60, 0.8]),
        };

        // Type badge pill.
        let badge_w = 90.0_f64;
        let badge_rect =
            RoundedRect::new(x + 8.0, row_y, x + 8.0 + badge_w, row_y + 22.0, 4.0);
        scene.fill(Fill::NonZero, Affine::IDENTITY, badge_color, None, &badge_rect);
        crate::dashboard::draw_text_pub(
            scene,
            x + 12.0,
            row_y + 16.0,
            type_label,
            Color::new([1.0, 1.0, 1.0, 1.0]),
            16.0,
            font_data,
        );

        // Content preview.
        crate::dashboard::draw_text_pub(
            scene,
            x + badge_w + 16.0,
            row_y + 16.0,
            &content_truncated,
            TEXT_PRIMARY,
            18.0,
            font_data,
        );

        // Timestamp (smaller, dimmed).
        crate::dashboard::draw_text_pub(
            scene,
            x + 12.0,
            row_y + LINE_HEIGHT - 6.0,
            &time_short,
            TEXT_SECONDARY,
            14.0,
            font_data,
        );

        row_y += LINE_HEIGHT + 4.0;
        if row_y + LINE_HEIGHT > y + h - 8.0 {
            break;
        }
    }
}

/// Draw the Active Subagents panel.
fn draw_agents_panel(
    scene: &mut Scene,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    state: Option<&crate::protocol::DashboardState>,
    font_data: Option<&FontData>,
) {
    // Panel background.
    let panel_rect = RoundedRect::new(x, y, x + w, y + h, CORNER_RADIUS);
    scene.fill(Fill::NonZero, Affine::IDENTITY, PANEL_BG, None, &panel_rect);

    use vello::kurbo::Stroke;
    scene.stroke(
        &Stroke::new(1.5),
        Affine::IDENTITY,
        PANEL_BORDER,
        None,
        &panel_rect,
    );

    // Section header.
    let header_rect = RoundedRect::new(x, y, x + w, y + 44.0, CORNER_RADIUS);
    scene.fill(Fill::NonZero, Affine::IDENTITY, ACCENT_GREEN, None, &header_rect);
    crate::dashboard::draw_text_pub(
        scene,
        x + 12.0,
        y + 30.0,
        "ACTIVE AGENTS",
        Color::new([1.0, 1.0, 1.0, 1.0]),
        22.0,
        font_data,
    );

    let agents = state
        .map(|s| s.subagent_list.agents.as_slice())
        .unwrap_or(&[]);

    let pending_count = state.map(|s| s.subagent_list.pending_count).unwrap_or(0);

    // Summary line.
    let summary = format!("{} pending agent(s)", pending_count);
    crate::dashboard::draw_text_pub(
        scene,
        x + 12.0,
        y + 62.0,
        &summary,
        TEXT_SECONDARY,
        20.0,
        font_data,
    );

    if agents.is_empty() {
        crate::dashboard::draw_text_pub(
            scene,
            x + 12.0,
            y + 90.0,
            "No active agents",
            TEXT_SECONDARY,
            22.0,
            font_data,
        );
        return;
    }

    let max_visible = ((h - 80.0) / 70.0) as usize;
    let agents_to_show = agents.iter().take(max_visible);

    let mut row_y = y + 74.0;
    for agent in agents_to_show {
        // Agent card background.
        let card_rect = RoundedRect::new(
            x + 8.0,
            row_y,
            x + w - 8.0,
            row_y + 62.0,
            5.0,
        );
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::new([1.0, 0.95, 0.85, 0.8]),
            None,
            &card_rect,
        );

        // Agent ID.
        let id_short: String = agent.id.chars().take(20).collect();
        crate::dashboard::draw_text_pub(
            scene,
            x + 16.0,
            row_y + 20.0,
            &id_short,
            TEXT_PRIMARY,
            20.0,
            font_data,
        );

        // Status badge.
        let status_color = match agent.status.as_str() {
            "running" | "in_progress" => ACCENT_GREEN,
            "failed" | "error" => Color::new([0.80, 0.20, 0.18, 1.0]),
            _ => ACCENT_AMBER,
        };
        let status_badge = RoundedRect::new(x + 16.0, row_y + 26.0, x + 16.0 + 80.0, row_y + 46.0, 4.0);
        scene.fill(Fill::NonZero, Affine::IDENTITY, status_color, None, &status_badge);
        crate::dashboard::draw_text_pub(
            scene,
            x + 20.0,
            row_y + 40.0,
            &agent.status,
            Color::new([1.0, 1.0, 1.0, 1.0]),
            16.0,
            font_data,
        );

        // Description (truncated).
        let desc_short: String = agent.description.chars().take(55).collect();
        crate::dashboard::draw_text_pub(
            scene,
            x + 106.0,
            row_y + 40.0,
            &desc_short,
            TEXT_SECONDARY,
            16.0,
            font_data,
        );

        // Elapsed time.
        if let Some(elapsed) = agent.elapsed_seconds {
            let elapsed_text = format_elapsed(elapsed);
            crate::dashboard::draw_text_pub(
                scene,
                x + w - 100.0,
                row_y + 20.0,
                &elapsed_text,
                TEXT_SECONDARY,
                16.0,
                font_data,
            );
        }

        row_y += 70.0;
        if row_y + 62.0 > y + h - 8.0 {
            break;
        }
    }
}

/// Format elapsed seconds as "Xm Ys" or "Xs".
fn format_elapsed(secs: u64) -> String {
    if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}
