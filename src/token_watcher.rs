//! File watcher for design token hot-reload.
//!
//! Watches `~/.config/bisque-computer/design.toml` and reloads on change.
//! This module is self-contained and can be integrated with the render pipeline
//! once the `DesignTokens` struct is available (Layer 1+2).
//!
//! # Usage
//!
//! ```rust,ignore
//! use token_watcher::TokenFileWatcher;
//!
//! let watcher = TokenFileWatcher::start(|toml_content| {
//!     println!("design.toml changed, new content: {}", toml_content.len());
//! }).expect("Failed to start token watcher");
//!
//! println!("Watching: {}", watcher.path().display());
//! ```

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Return the config directory for bisque-computer: `~/.config/bisque-computer/`.
///
/// Uses `$XDG_CONFIG_HOME` if set, otherwise falls back to `$HOME/.config/bisque-computer/`.
/// On macOS without XDG, still uses `.config` (not `Library/Application Support`)
/// so the TOML file is easy to find and edit in a terminal.
pub fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let mut p = PathBuf::from(xdg);
        p.push("bisque-computer");
        return p;
    }
    let mut p = home_dir();
    p.push(".config");
    p.push("bisque-computer");
    p
}

/// Return the full path to `design.toml`.
pub fn design_toml_path() -> PathBuf {
    let mut p = config_dir();
    p.push("design.toml");
    p
}

/// Return the user's home directory, falling back to `/tmp` if `$HOME` is unset.
fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

// ---------------------------------------------------------------------------
// File I/O helpers
// ---------------------------------------------------------------------------

/// Write the default TOML content to `path` if the file does not already exist.
/// Creates parent directories as needed.
pub fn ensure_default_toml(path: &Path, default_content: &str) {
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("token_watcher: failed to create config dir {}: {e}", parent.display());
            return;
        }
    }
    if let Err(e) = std::fs::write(path, default_content) {
        eprintln!("token_watcher: failed to write default design.toml at {}: {e}", path.display());
    }
}

/// Read the TOML file content from disk.
pub fn load_tokens_from_file(path: &Path) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}

// ---------------------------------------------------------------------------
// Low-level watcher
// ---------------------------------------------------------------------------

/// Spawn a file watcher on the *parent directory* of `path`.
///
/// Editors like vim/nano write to a temp file then rename, which means watching
/// the file directly misses changes. Watching the parent directory and filtering
/// by filename handles this correctly.
///
/// `on_change` is called whenever the target file is created or modified.
pub fn spawn_watcher<F>(path: PathBuf, on_change: F) -> notify::Result<RecommendedWatcher>
where
    F: Fn() + Send + 'static,
{
    let target_filename = path
        .file_name()
        .expect("design.toml path must have a filename")
        .to_os_string();

    let parent = path
        .parent()
        .expect("design.toml path must have a parent directory")
        .to_path_buf();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        match res {
            Ok(event) => {
                // Only react to Create or Modify events.
                let dominated = matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_)
                );
                if !dominated {
                    return;
                }

                // Check if any of the affected paths match our target filename.
                let affects_target = event.paths.iter().any(|p| {
                    p.file_name()
                        .map(|f| f == target_filename)
                        .unwrap_or(false)
                });

                if affects_target {
                    on_change();
                }
            }
            Err(e) => {
                eprintln!("token_watcher: watch error: {e}");
            }
        }
    })?;

    watcher.watch(&parent, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

// ---------------------------------------------------------------------------
// High-level TokenFileWatcher
// ---------------------------------------------------------------------------

/// Owns a file-system watcher for `design.toml` and provides a clean API
/// for the rest of the application.
pub struct TokenFileWatcher {
    _watcher: RecommendedWatcher,
    path: PathBuf,
}

impl TokenFileWatcher {
    /// Start watching `design.toml`. Calls `on_reload` with the new TOML content
    /// whenever the file changes. The callback should parse the TOML and update
    /// shared state.
    ///
    /// If `design.toml` does not exist, a well-commented default is written first.
    pub fn start<F>(on_reload: F) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: Fn(String) + Send + 'static,
    {
        let path = design_toml_path();

        // Ensure the default TOML exists so the user has something to edit.
        ensure_default_toml(&path, &default_toml_content());

        let watched_path = path.clone();
        let watcher = spawn_watcher(path.clone(), move || {
            match load_tokens_from_file(&watched_path) {
                Ok(content) => on_reload(content),
                Err(e) => eprintln!(
                    "token_watcher: failed to read {}: {e}",
                    watched_path.display()
                ),
            }
        })?;

        Ok(Self {
            _watcher: watcher,
            path,
        })
    }

    /// Get the path being watched.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write content to the watched file (for "save" command in REPL).
    pub fn write_toml(&self, content: &str) -> std::io::Result<()> {
        std::fs::write(&self.path, content)
    }
}

// ---------------------------------------------------------------------------
// Default TOML template
// ---------------------------------------------------------------------------

/// Generate a well-commented default TOML file with all design tokens.
///
/// Values match the compiled defaults in the design system so that the file
/// is a no-op on first load but gives the user a complete reference.
pub fn default_toml_content() -> String {
    r#"# Bisque Design System — edit this file to change the interface in real-time
# Any missing values use compiled defaults. Delete a line to reset it.
#
# Changes are detected automatically — just save the file.

[background]
r = 1.0       # Red channel (0.0-1.0)
g = 0.894     # Green channel
b = 0.769     # Blue channel — bisque beige

[ink]
# Black text at varying opacities on bisque background
primary = 1.0       # Headings, primary content
section = 0.80      # Section titles
body = 0.70         # Body text
secondary = 0.50    # Supplementary text
annotation = 0.40   # Timestamps, metadata
rule = 0.15         # Hairline dividers
ghost = 0.08        # Disabled, placeholder

[type_scale]
base = 18.0         # Body text size in pixels
ratio = 1.333       # Perfect Fourth — each step is 1.333x larger

[line_height]
body = 1.5          # Body text line-height ratio
heading = 1.2       # Headings (24-42px)
display = 1.05      # Display text (56px+)
caption = 1.4       # Captions and small text

[spacing]
baseline = 28.0     # Baseline grid unit (all spacing is a multiple of this)

[margins]
left_frac = 0.1111  # Left margin as fraction of viewport width (1/9)
right_frac = 0.2222 # Right margin (2/9)
top_frac = 0.1111   # Top margin (1/9)
bottom_frac = 0.2222 # Bottom margin (2/9)
left_min = 48.0     # Minimum left margin in pixels

[grid]
gutter = 24.0       # Grid gutter width in pixels

[rules]
thickness = 0.5     # Horizontal rule thickness in pixels

[tracking]
caps = 0.08         # Letter-spacing for ALL CAPS (em units)
display = -0.01     # Letter-spacing for display text (tighter)
small = 0.02        # Letter-spacing for small annotations

[animation]
spring_k = 0.2      # Spring constant for screen transitions
snap_threshold = 0.001  # Snap distance for animation completion
cursor_blink_ms = 500   # Cursor blink interval

[terminal]
font_size = 28.0    # Terminal font size in pixels
pad_cells = 1       # Terminal horizontal padding in cell widths
"#
    .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_config_path() {
        let path = design_toml_path();
        let path_str = path.to_string_lossy();
        assert!(
            path_str.ends_with("bisque-computer/design.toml"),
            "Expected path to end with bisque-computer/design.toml, got: {path_str}"
        );
    }

    #[test]
    fn test_default_toml_is_valid() {
        let content = default_toml_content();
        let parsed: Result<toml::Value, _> = toml::from_str(&content);
        assert!(
            parsed.is_ok(),
            "Default TOML failed to parse: {:?}",
            parsed.err()
        );

        // Verify key sections exist.
        let value = parsed.unwrap();
        let table = value.as_table().expect("TOML root should be a table");
        for section in &[
            "background",
            "ink",
            "type_scale",
            "line_height",
            "spacing",
            "margins",
            "grid",
            "rules",
            "tracking",
            "animation",
            "terminal",
        ] {
            assert!(
                table.contains_key(*section),
                "Missing section: [{section}]"
            );
        }
    }

    #[test]
    fn test_ensure_default_creates_file() {
        let dir = std::env::temp_dir().join(format!(
            "bisque_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("design.toml");

        assert!(!path.exists());
        ensure_default_toml(&path, "# test content\nfoo = 42\n");
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("foo = 42"));

        // Cleanup.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_ensure_default_does_not_overwrite() {
        let dir = std::env::temp_dir().join(format!(
            "bisque_test_no_overwrite_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("design.toml");

        // Write existing content.
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "# user customized").unwrap();
        drop(f);

        // Call ensure_default — should NOT overwrite.
        ensure_default_toml(&path, "# default content\n");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.contains("user customized"),
            "ensure_default_toml should not overwrite existing file"
        );

        // Cleanup.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_tokens_from_file() {
        let dir = std::env::temp_dir().join(format!(
            "bisque_test_load_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("design.toml");
        std::fs::write(&path, "[ink]\nprimary = 0.9\n").unwrap();

        let content = load_tokens_from_file(&path).unwrap();
        assert!(content.contains("primary = 0.9"));

        // Cleanup.
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_tokens_missing_file() {
        let path = std::env::temp_dir().join("nonexistent_bisque_design.toml");
        let result = load_tokens_from_file(&path);
        assert!(result.is_err());
    }
}
