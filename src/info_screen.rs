//! Info/Memory screen — the center screen in the 3-panel layout.
//!
//! Shows recent memory events and active subagents pulled from
//! the live WebSocket state (`DashboardState`).
//!
//! Typography-first design: no boxes, no panels, no badges.
//! Hierarchy communicated through font size, opacity, and whitespace.

use vello::Scene;
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill, FontData};

use crate::ws_client::SharedInstances;

// --- Color palette (typography-only) ---
const BG_COLOR: Color = Color::new([1.0, 0.894, 0.769, 1.0]);
const TEXT_PRIMARY: Color = Color::new([0.0, 0.0, 0.0, 1.0]);
const TEXT_SECONDARY: Color = Color::new([0.0, 0.0, 0.0, 0.85]);
const TEXT_SECTION: Color = Color::new([0.0, 0.0, 0.0, 0.92]);
const TEXT_ANNOTATION: Color = Color::new([0.0, 0.0, 0.0, 0.75]);
const RULE_COLOR: Color = Color::new([0.0, 0.0, 0.0, 0.15]);

// --- Typography sizes ---
const TITLE_SIZE: f64 = 44.0;
const SECTION_SIZE: f64 = 18.0;
const DATA_SECONDARY_SIZE: f64 = 20.0;
const ANNOTATION_SIZE: f64 = 16.0;

// --- Layout ---
const LEFT_MARGIN: f64 = 48.0;
const LINE_HEIGHT_FACTOR: f64 = 1.4;
const RULE_THICKNESS: f64 = 0.5;
const COL_GAP: f64 = 48.0;

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
    _tokens: &crate::design::DesignTokens,
) {
    // Background fill.
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        BG_COLOR,
        None,
        &Rect::new(0.0, 0.0, width, height),
    );

    // Page title
    let title_y = 56.0;
    crate::dashboard::draw_text_pub(
        scene, LEFT_MARGIN, title_y, "Memory & Agents", TEXT_PRIMARY, TITLE_SIZE, font_data,
    );

    let content_top = title_y + 32.0;
    let content_height = height - content_top - LEFT_MARGIN;
    let half_w = (width - LEFT_MARGIN * 2.0 - COL_GAP) / 2.0;

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

    // Left column: Recent Memory Events.
    draw_memory_column(
        scene,
        LEFT_MARGIN,
        content_top,
        half_w,
        content_height,
        instance_state.as_ref(),
        font_data,
    );

    // Right column: Active Subagents.
    draw_agents_column(
        scene,
        LEFT_MARGIN + half_w + COL_GAP,
        content_top,
        half_w,
        content_height,
        instance_state.as_ref(),
        font_data,
    );
}

/// Draw a section header: title text + thin rule underneath.
/// Returns y position for content below.
fn draw_section_header(
    scene: &mut Scene,
    x: f64,
    y: f64,
    w: f64,
    title: &str,
    font_data: Option<&FontData>,
) -> f64 {
    crate::dashboard::draw_text_pub(scene, x, y, title, TEXT_SECTION, SECTION_SIZE, font_data);

    let rule_y = y + 6.0;
    let rule_rect = Rect::new(x, rule_y, x + w, rule_y + RULE_THICKNESS);
    scene.fill(Fill::NonZero, Affine::IDENTITY, RULE_COLOR, None, &rule_rect);

    y + SECTION_SIZE * LINE_HEIGHT_FACTOR + 4.0
}

/// Draw the Recent Memory Events column.
fn draw_memory_column(
    scene: &mut Scene,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    state: Option<&crate::protocol::DashboardState>,
    font_data: Option<&FontData>,
) {
    let mut cursor_y = y;

    cursor_y = draw_section_header(scene, x, cursor_y, w, "Recent Memory", font_data);

    let events = state
        .map(|s| s.memory.recent_events.as_slice())
        .unwrap_or(&[]);

    if events.is_empty() {
        crate::dashboard::draw_text_pub(
            scene,
            x,
            cursor_y,
            "No recent memory events",
            TEXT_ANNOTATION,
            DATA_SECONDARY_SIZE,
            font_data,
        );
        return;
    }

    let line_height = DATA_SECONDARY_SIZE * LINE_HEIGHT_FACTOR;
    let max_visible = ((h - (cursor_y - y)) / (line_height + ANNOTATION_SIZE * LINE_HEIGHT_FACTOR)) as usize;
    let events_to_show = events.iter().rev().take(max_visible);

    for event in events_to_show {
        let type_label = &event.event_type;
        let content_truncated: String = event.content.chars().take(60).collect();
        let time_short: String = event.timestamp.chars().take(19).collect();

        // Event type as bracketed text annotation: "[decision]"
        let type_tag = format!("[{}]", type_label);
        crate::dashboard::draw_text_pub(
            scene,
            x,
            cursor_y,
            &type_tag,
            TEXT_ANNOTATION,
            ANNOTATION_SIZE,
            font_data,
        );

        // Content preview
        crate::dashboard::draw_text_pub(
            scene,
            x + 90.0,
            cursor_y,
            &content_truncated,
            TEXT_PRIMARY,
            DATA_SECONDARY_SIZE,
            font_data,
        );

        // Timestamp below, smaller and dimmer
        cursor_y += DATA_SECONDARY_SIZE * LINE_HEIGHT_FACTOR;
        crate::dashboard::draw_text_pub(
            scene,
            x + 90.0,
            cursor_y,
            &time_short,
            TEXT_ANNOTATION,
            ANNOTATION_SIZE,
            font_data,
        );

        cursor_y += ANNOTATION_SIZE * LINE_HEIGHT_FACTOR + 8.0;
        if cursor_y + line_height > y + h {
            break;
        }
    }
}

/// Draw the Active Subagents column.
fn draw_agents_column(
    scene: &mut Scene,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    state: Option<&crate::protocol::DashboardState>,
    font_data: Option<&FontData>,
) {
    let mut cursor_y = y;

    cursor_y = draw_section_header(scene, x, cursor_y, w, "Active Agents", font_data);

    let agents = state
        .map(|s| s.subagent_list.agents.as_slice())
        .unwrap_or(&[]);

    let pending_count = state.map(|s| s.subagent_list.pending_count).unwrap_or(0);

    // Summary line
    let summary = format!("{} pending", pending_count);
    crate::dashboard::draw_text_pub(
        scene,
        x,
        cursor_y,
        &summary,
        TEXT_SECONDARY,
        DATA_SECONDARY_SIZE,
        font_data,
    );
    cursor_y += DATA_SECONDARY_SIZE * LINE_HEIGHT_FACTOR + 8.0;

    if agents.is_empty() {
        crate::dashboard::draw_text_pub(
            scene,
            x,
            cursor_y,
            "No active agents",
            TEXT_ANNOTATION,
            DATA_SECONDARY_SIZE,
            font_data,
        );
        return;
    }

    let entry_height = DATA_SECONDARY_SIZE * LINE_HEIGHT_FACTOR * 2.0 + 12.0;
    let max_visible = ((h - (cursor_y - y)) / entry_height) as usize;
    let agents_to_show = agents.iter().take(max_visible);

    for agent in agents_to_show {
        // Agent ID
        let id_short: String = agent.id.chars().take(20).collect();
        crate::dashboard::draw_text_pub(
            scene,
            x,
            cursor_y,
            &id_short,
            TEXT_PRIMARY,
            DATA_SECONDARY_SIZE,
            font_data,
        );

        // Elapsed time on the right
        if let Some(elapsed) = agent.elapsed_seconds {
            let elapsed_text = format_elapsed(elapsed);
            let elapsed_w = elapsed_text.len() as f64 * 10.0;
            crate::dashboard::draw_text_pub(
                scene,
                (x + w - elapsed_w).max(x + 200.0),
                cursor_y,
                &elapsed_text,
                TEXT_ANNOTATION,
                ANNOTATION_SIZE,
                font_data,
            );
        }

        cursor_y += DATA_SECONDARY_SIZE * LINE_HEIGHT_FACTOR;

        // Status as text + description
        let status_text = format!("{}", agent.status);
        crate::dashboard::draw_text_pub(
            scene,
            x,
            cursor_y,
            &status_text,
            TEXT_SECONDARY,
            ANNOTATION_SIZE,
            font_data,
        );

        // Description (truncated)
        let desc_short: String = agent.description.chars().take(55).collect();
        crate::dashboard::draw_text_pub(
            scene,
            x + 80.0,
            cursor_y,
            &desc_short,
            TEXT_ANNOTATION,
            ANNOTATION_SIZE,
            font_data,
        );

        cursor_y += ANNOTATION_SIZE * LINE_HEIGHT_FACTOR + 12.0;

        if cursor_y + entry_height > y + h {
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
