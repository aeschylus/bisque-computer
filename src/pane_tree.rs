//! Binary split tree for tiling terminal panes.
//!
//! Provides iTerm2-style pane splitting, closing, and focus cycling.
//! Each leaf node owns a `TerminalPane` backed by either a local PTY
//! or a sandboxed process via macOS `sandbox-exec`.

use std::path::PathBuf;

use tracing::{info, warn};
use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill};
use vello::Scene;

use crate::terminal::TerminalPane;

/// The sandbox profile embedded in the binary at compile time.
const SANDBOX_PROFILE: &str = include_str!("../sandbox/claude-sandbox.sb");

/// Selects how new terminal panes are created.
#[derive(Clone)]
pub enum TerminalBackend {
    /// Local PTY shell (existing behavior).
    Local,
    /// Sandboxed process via macOS sandbox-exec.
    Sandboxed {
        project_dir: PathBuf,
        profile_path: PathBuf,
    },
}

impl TerminalBackend {
    /// Create a `Sandboxed` backend for the given project directory.
    ///
    /// Writes the embedded sandbox profile to a temp file and returns the backend.
    /// Returns `Local` as a fallback if the temp file cannot be created.
    pub fn sandboxed(project_dir: PathBuf) -> Self {
        let profile_path = std::env::temp_dir().join("bisque-claude-sandbox.sb");
        match std::fs::write(&profile_path, SANDBOX_PROFILE) {
            Ok(()) => {
                info!(
                    project_dir = %project_dir.display(),
                    profile = %profile_path.display(),
                    "Sandbox profile written"
                );
                Self::Sandboxed { project_dir, profile_path }
            }
            Err(e) => {
                warn!("Failed to write sandbox profile to {}: {} — falling back to local PTY",
                    profile_path.display(), e);
                Self::Local
            }
        }
    }
}

const MIN_PANE_WIDTH: f64 = 80.0;
const MIN_PANE_HEIGHT: f64 = 40.0;
const SEPARATOR_PX: f64 = 2.0;
const SEPARATOR_COLOR: Color = Color::new([0.25, 0.25, 0.30, 1.0]);
const FOCUS_BORDER_COLOR: Color = Color::new([0.85, 0.65, 0.20, 0.85]);
const FOCUS_BORDER_PX: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusChild {
    First,
    Second,
}

pub enum PaneNode {
    Leaf(TerminalPane),
    Split {
        direction: SplitDirection,
        ratio: f64,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

/// The tree uses `Option<PaneNode>` at the root so we can take() ownership
/// for transformations like split and close.
pub struct PaneTree {
    root: Option<PaneNode>,
    focus_path: Vec<FocusChild>,
    /// Backend configuration for spawning new panes.
    backend: TerminalBackend,
}

impl PaneTree {
    /// Create a new pane tree with a single terminal pane.
    ///
    /// Uses the `Local` backend by default (existing behavior).
    pub fn new(width: f64, height: f64) -> Option<Self> {
        Self::with_backend(width, height, TerminalBackend::Local)
    }

    /// Create a new pane tree with a specific backend.
    pub fn with_backend(width: f64, height: f64, backend: TerminalBackend) -> Option<Self> {
        let pane = spawn_pane_for_backend(&backend, width, height)?;
        Some(Self {
            root: Some(PaneNode::Leaf(pane)),
            focus_path: Vec::new(),
            backend,
        })
    }

    pub fn split_focused(&mut self, direction: SplitDirection, width: f64, height: f64) {
        let Some(root) = self.root.take() else { return };

        let focused_rect = rect_at_path(&root, &self.focus_path, 0.0, 0.0, width, height);
        let (first_w, first_h, second_w, second_h) = match direction {
            SplitDirection::Vertical => {
                let half = (focused_rect.width() - SEPARATOR_PX) / 2.0;
                (half, focused_rect.height(), half, focused_rect.height())
            }
            SplitDirection::Horizontal => {
                let half = (focused_rect.height() - SEPARATOR_PX) / 2.0;
                (focused_rect.width(), half, focused_rect.width(), half)
            }
        };

        if first_w < MIN_PANE_WIDTH || second_w < MIN_PANE_WIDTH
            || first_h < MIN_PANE_HEIGHT || second_h < MIN_PANE_HEIGHT
        {
            warn!("Refusing to split: resulting panes would be too small");
            self.root = Some(root);
            return;
        }

        let new_pane = match spawn_pane_for_backend(&self.backend, second_w, second_h) {
            Some(pane) => pane,
            None => {
                warn!("Failed to spawn new terminal pane for split");
                self.root = Some(root);
                return;
            }
        };

        let new_root = transform_split(root, &self.focus_path, 0, direction, new_pane, first_w, first_h);
        self.root = Some(new_root);
        self.focus_path.push(FocusChild::Second);
    }

    /// Close the focused pane. Returns `false` if this was the last pane.
    pub fn close_focused(&mut self) -> bool {
        if self.focus_path.is_empty() {
            // Root is a single leaf — last pane.
            return false;
        }

        let Some(root) = self.root.take() else { return false };
        let parent_path_len = self.focus_path.len() - 1;
        let which_child = self.focus_path[parent_path_len];

        let new_root = transform_close(root, &self.focus_path, 0, which_child);
        self.root = Some(new_root);

        // Update focus to first leaf under the replacement node.
        self.focus_path.truncate(parent_path_len);
        if let Some(ref root) = self.root {
            let node = node_at_path(root, &self.focus_path);
            descend_first_leaf(node, &mut self.focus_path);
        }

        true
    }

    pub fn focused_mut(&mut self) -> Option<&mut TerminalPane> {
        let root = self.root.as_mut()?;
        let node = node_at_path_mut(root, &self.focus_path);
        match node {
            PaneNode::Leaf(term) => Some(term),
            _ => None,
        }
    }

    pub fn focused(&self) -> Option<&TerminalPane> {
        let root = self.root.as_ref()?;
        let node = node_at_path(root, &self.focus_path);
        match node {
            PaneNode::Leaf(term) => Some(term),
            _ => None,
        }
    }

    pub fn cycle_focus(&mut self, forward: bool) {
        let Some(ref root) = self.root else { return };
        let mut leaves: Vec<Vec<FocusChild>> = Vec::new();
        collect_leaf_paths(root, &mut Vec::new(), &mut leaves);
        if leaves.len() <= 1 { return; }
        let idx = leaves.iter().position(|p| *p == self.focus_path).unwrap_or(0);
        let next = if forward {
            (idx + 1) % leaves.len()
        } else {
            (idx + leaves.len() - 1) % leaves.len()
        };
        self.focus_path = leaves[next].clone();
    }

    pub fn drain_all_output(&mut self) {
        if let Some(ref mut root) = self.root { drain_recursive(root); }
    }

    pub fn resize_all(&mut self, width: f64, height: f64) {
        if let Some(ref mut root) = self.root { resize_recursive(root, width, height); }
    }

    pub fn render_into_scene(&self, scene: &mut Scene, x: f64, y: f64, w: f64, h: f64) {
        let Some(ref root) = self.root else { return };
        let multi = count_leaves(root) > 1;
        render_recursive(root, scene, x, y, w, h, &self.focus_path, &[], multi);
    }

    pub fn pane_count(&self) -> usize {
        self.root.as_ref().map_or(0, count_leaves)
    }
}

// ---------------------------------------------------------------------------
// Tree transformations (take ownership, return new tree)
// ---------------------------------------------------------------------------

/// Replace the leaf at `path[depth..]` with a Split containing the original + new pane.
fn transform_split(
    node: PaneNode,
    path: &[FocusChild],
    depth: usize,
    direction: SplitDirection,
    new_pane: TerminalPane,
    first_w: f64,
    first_h: f64,
) -> PaneNode {
    if depth == path.len() {
        // This is the focused leaf — wrap it in a Split.
        let mut original = node;
        if let PaneNode::Leaf(ref mut term) = original {
            term.resize(first_w, first_h);
        }
        return PaneNode::Split {
            direction,
            ratio: 0.5,
            first: Box::new(original),
            second: Box::new(PaneNode::Leaf(new_pane)),
        };
    }

    match node {
        PaneNode::Split { direction: d, ratio, first, second } => {
            match path[depth] {
                FocusChild::First => PaneNode::Split {
                    direction: d,
                    ratio,
                    first: Box::new(transform_split(*first, path, depth + 1, direction, new_pane, first_w, first_h)),
                    second,
                },
                FocusChild::Second => PaneNode::Split {
                    direction: d,
                    ratio,
                    first,
                    second: Box::new(transform_split(*second, path, depth + 1, direction, new_pane, first_w, first_h)),
                },
            }
        }
        leaf @ PaneNode::Leaf(_) => leaf, // path invalid, return unchanged
    }
}

/// Remove the leaf at `path` and replace its parent Split with the surviving sibling.
fn transform_close(
    node: PaneNode,
    path: &[FocusChild],
    depth: usize,
    which_child: FocusChild,
) -> PaneNode {
    if depth == path.len() - 1 {
        // This node is the parent Split of the leaf being closed.
        match node {
            PaneNode::Split { first, second, .. } => match which_child {
                FocusChild::First => *second,  // close first, keep second
                FocusChild::Second => *first,  // close second, keep first
            },
            other => other, // shouldn't happen
        }
    } else {
        match node {
            PaneNode::Split { direction, ratio, first, second } => match path[depth] {
                FocusChild::First => PaneNode::Split {
                    direction,
                    ratio,
                    first: Box::new(transform_close(*first, path, depth + 1, which_child)),
                    second,
                },
                FocusChild::Second => PaneNode::Split {
                    direction,
                    ratio,
                    first,
                    second: Box::new(transform_close(*second, path, depth + 1, which_child)),
                },
            },
            other => other,
        }
    }
}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

fn node_at_path_mut<'a>(root: &'a mut PaneNode, path: &[FocusChild]) -> &'a mut PaneNode {
    let mut current = root;
    for step in path {
        current = match current {
            PaneNode::Split { first, second, .. } => match step {
                FocusChild::First => first.as_mut(),
                FocusChild::Second => second.as_mut(),
            },
            PaneNode::Leaf(_) => return current,
        };
    }
    current
}

fn node_at_path<'a>(root: &'a PaneNode, path: &[FocusChild]) -> &'a PaneNode {
    let mut current = root;
    for step in path {
        current = match current {
            PaneNode::Split { first, second, .. } => match step {
                FocusChild::First => first.as_ref(),
                FocusChild::Second => second.as_ref(),
            },
            PaneNode::Leaf(_) => return current,
        };
    }
    current
}

fn descend_first_leaf(node: &PaneNode, path: &mut Vec<FocusChild>) {
    match node {
        PaneNode::Leaf(_) => {}
        PaneNode::Split { first, .. } => {
            path.push(FocusChild::First);
            descend_first_leaf(first, path);
        }
    }
}

fn collect_leaf_paths(node: &PaneNode, path: &mut Vec<FocusChild>, out: &mut Vec<Vec<FocusChild>>) {
    match node {
        PaneNode::Leaf(_) => out.push(path.clone()),
        PaneNode::Split { first, second, .. } => {
            path.push(FocusChild::First);
            collect_leaf_paths(first, path, out);
            path.pop();
            path.push(FocusChild::Second);
            collect_leaf_paths(second, path, out);
            path.pop();
        }
    }
}

// ---------------------------------------------------------------------------
// Recursive operations
// ---------------------------------------------------------------------------

fn drain_recursive(node: &mut PaneNode) {
    match node {
        PaneNode::Leaf(term) => term.drain_output(),
        PaneNode::Split { first, second, .. } => { drain_recursive(first); drain_recursive(second); }
    }
}

fn resize_recursive(node: &mut PaneNode, w: f64, h: f64) {
    match node {
        PaneNode::Leaf(term) => term.resize(w, h),
        PaneNode::Split { direction, ratio, first, second } => {
            let (r1, r2) = split_dims(w, h, *direction, *ratio);
            resize_recursive(first, r1.0, r1.1);
            resize_recursive(second, r2.0, r2.1);
        }
    }
}

fn render_recursive(
    node: &PaneNode, scene: &mut Scene,
    x: f64, y: f64, w: f64, h: f64,
    focus_path: &[FocusChild], current_path: &[FocusChild],
    multi: bool,
) {
    match node {
        PaneNode::Leaf(term) => {
            term.render_into_scene(scene, x, y, w, h);
            if multi && current_path == focus_path {
                draw_focus_border(scene, x, y, w, h);
            }
        }
        PaneNode::Split { direction, ratio, first, second } => {
            let (r1, r2) = split_dims(w, h, *direction, *ratio);
            let mut p1 = current_path.to_vec(); p1.push(FocusChild::First);
            render_recursive(first, scene, x, y, r1.0, r1.1, focus_path, &p1, multi);

            let sep = separator_rect(x, y, w, h, *direction, *ratio);
            scene.fill(Fill::NonZero, Affine::IDENTITY, SEPARATOR_COLOR, None, &sep);

            let (x2, y2) = second_origin(x, y, *direction, r1);
            let mut p2 = current_path.to_vec(); p2.push(FocusChild::Second);
            render_recursive(second, scene, x2, y2, r2.0, r2.1, focus_path, &p2, multi);
        }
    }
}

fn count_leaves(node: &PaneNode) -> usize {
    match node {
        PaneNode::Leaf(_) => 1,
        PaneNode::Split { first, second, .. } => count_leaves(first) + count_leaves(second),
    }
}

fn rect_at_path(node: &PaneNode, path: &[FocusChild], x: f64, y: f64, w: f64, h: f64) -> Rect {
    if path.is_empty() { return Rect::new(x, y, x + w, y + h); }
    match node {
        PaneNode::Split { direction, ratio, first, second } => {
            let (r1, r2) = split_dims(w, h, *direction, *ratio);
            match path[0] {
                FocusChild::First => rect_at_path(first, &path[1..], x, y, r1.0, r1.1),
                FocusChild::Second => {
                    let (x2, y2) = second_origin(x, y, *direction, r1);
                    rect_at_path(second, &path[1..], x2, y2, r2.0, r2.1)
                }
            }
        }
        PaneNode::Leaf(_) => Rect::new(x, y, x + w, y + h),
    }
}

// ---------------------------------------------------------------------------
// Backend-aware pane spawning
// ---------------------------------------------------------------------------

/// Spawn a terminal pane using the given backend.
fn spawn_pane_for_backend(
    backend: &TerminalBackend,
    width: f64,
    height: f64,
) -> Option<TerminalPane> {
    match backend {
        TerminalBackend::Local => TerminalPane::spawn(width, height),
        TerminalBackend::Sandboxed { project_dir, profile_path } => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            let tmpdir = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_string());
            let home_npm = format!("{}/.npm", home);
            let home_config = format!("{}/.config", home);
            let project_str = project_dir.to_string_lossy().to_string();
            let profile_str = profile_path.to_string_lossy().to_string();

            let d_project = format!("PROJECT_DIR={}", project_str);
            let d_tmpdir = format!("TMPDIR={}", tmpdir);
            let d_home = format!("HOME={}", home);
            let d_npm = format!("HOME_NPM={}", home_npm);
            let d_config = format!("HOME_CONFIG={}", home_config);

            // Spawn a login shell inside the sandbox that execs claude.
            // This ensures PATH, env, and shell profile are properly set up.
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
            let exec_cmd = format!(
                "cd {} && exec claude --dangerously-skip-permissions",
                shell_escape(&project_str),
            );

            let args: Vec<&str> = vec![
                "-D", &d_project,
                "-D", &d_tmpdir,
                "-D", &d_home,
                "-D", &d_npm,
                "-D", &d_config,
                "-f", &profile_str,
                &shell,
                "-l", "-c", &exec_cmd,
            ];

            TerminalPane::spawn_command(
                width,
                height,
                "sandbox-exec",
                &args,
                &[],
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

fn split_dims(w: f64, h: f64, dir: SplitDirection, ratio: f64) -> ((f64, f64), (f64, f64)) {
    match dir {
        SplitDirection::Vertical => {
            let fw = ((w - SEPARATOR_PX) * ratio).floor();
            ((fw, h), (w - fw - SEPARATOR_PX, h))
        }
        SplitDirection::Horizontal => {
            let fh = ((h - SEPARATOR_PX) * ratio).floor();
            ((w, fh), (w, h - fh - SEPARATOR_PX))
        }
    }
}

fn second_origin(x: f64, y: f64, dir: SplitDirection, r1: (f64, f64)) -> (f64, f64) {
    match dir {
        SplitDirection::Vertical => (x + r1.0 + SEPARATOR_PX, y),
        SplitDirection::Horizontal => (x, y + r1.1 + SEPARATOR_PX),
    }
}

fn separator_rect(x: f64, y: f64, w: f64, h: f64, dir: SplitDirection, ratio: f64) -> Rect {
    match dir {
        SplitDirection::Vertical => {
            let fw = ((w - SEPARATOR_PX) * ratio).floor();
            Rect::new(x + fw, y, x + fw + SEPARATOR_PX, y + h)
        }
        SplitDirection::Horizontal => {
            let fh = ((h - SEPARATOR_PX) * ratio).floor();
            Rect::new(x, y + fh, x + w, y + fh + SEPARATOR_PX)
        }
    }
}

fn draw_focus_border(scene: &mut Scene, x: f64, y: f64, w: f64, h: f64) {
    let b = FOCUS_BORDER_PX;
    scene.fill(Fill::NonZero, Affine::IDENTITY, FOCUS_BORDER_COLOR, None, &Rect::new(x, y, x + w, y + b));
    scene.fill(Fill::NonZero, Affine::IDENTITY, FOCUS_BORDER_COLOR, None, &Rect::new(x, y + h - b, x + w, y + h));
    scene.fill(Fill::NonZero, Affine::IDENTITY, FOCUS_BORDER_COLOR, None, &Rect::new(x, y, x + b, y + h));
    scene.fill(Fill::NonZero, Affine::IDENTITY, FOCUS_BORDER_COLOR, None, &Rect::new(x + w - b, y, x + w, y + h));
}

/// Escape a string for use in a shell command.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
