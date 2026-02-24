//! Application directory structure for bisque-computer.
//!
//! Provides a single `BisquePaths` struct that resolves all standard directories
//! and ensures they exist on first launch. Follows macOS conventions:
//!
//! - Config:    `~/.config/bisque-computer/`  (human-editable, XDG-style)
//! - Data:      `~/Library/Application Support/com.fullyparsed.bisque-computer/`
//! - Cache:     `~/Library/Caches/com.fullyparsed.bisque-computer/`
//! - Logs:      `~/Library/Logs/bisque-computer/`  (existing convention)
//!
//! On non-macOS, falls back to XDG paths.

use std::path::{Path, PathBuf};
use tracing::{info, warn};

const BUNDLE_ID: &str = "com.fullyparsed.bisque-computer";
const APP_NAME: &str = "bisque-computer";

/// All resolved application directory paths.
#[derive(Debug, Clone)]
pub struct BisquePaths {
    /// Human-editable config: `~/.config/bisque-computer/`
    pub config: PathBuf,
    /// Machine-managed application data root
    pub data: PathBuf,
    /// User project workspaces
    pub projects: PathBuf,
    /// Docker/OCI container images
    pub containers: PathBuf,
    /// VM disk images
    pub vms: PathBuf,
    /// ML models (whisper, etc.)
    pub models: PathBuf,
    /// SQLite databases, indexes
    pub db: PathBuf,
    /// Regenerable cache data
    pub cache: PathBuf,
    /// Application logs
    pub logs: PathBuf,
}

impl BisquePaths {
    /// Resolve all paths from the user's home directory.
    /// Does not create any directories — call `ensure()` for that.
    pub fn resolve() -> Option<Self> {
        let home = std::env::var("HOME").ok().map(PathBuf::from)?;

        let config = resolve_config_dir(&home);
        let data = resolve_data_dir(&home);
        let cache = resolve_cache_dir(&home);
        let logs = resolve_log_dir(&home);

        Some(Self {
            config,
            projects: data.join("projects"),
            containers: data.join("containers"),
            vms: data.join("vms"),
            models: data.join("models"),
            db: data.join("db"),
            data,
            cache,
            logs,
        })
    }

    /// Create all directories that don't already exist.
    /// Applies Time Machine exclusions to large/regenerable directories on macOS.
    pub fn ensure(&self) -> std::io::Result<()> {
        let dirs = [
            &self.config,
            &self.data,
            &self.projects,
            &self.containers,
            &self.vms,
            &self.models,
            &self.db,
            &self.cache,
            &self.logs,
        ];

        for dir in &dirs {
            std::fs::create_dir_all(dir)?;
            info!("ensured directory: {}", dir.display());
        }

        // Exclude large/regenerable directories from Time Machine
        #[cfg(target_os = "macos")]
        {
            let tm_exclude = [&self.containers, &self.vms, &self.models, &self.cache];
            for dir in &tm_exclude {
                exclude_from_time_machine(dir);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Platform-specific path resolution
// ---------------------------------------------------------------------------

fn resolve_config_dir(home: &Path) -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join(APP_NAME)
    } else {
        home.join(".config").join(APP_NAME)
    }
}

#[cfg(target_os = "macos")]
fn resolve_data_dir(home: &Path) -> PathBuf {
    home.join("Library")
        .join("Application Support")
        .join(BUNDLE_ID)
}

#[cfg(not(target_os = "macos"))]
fn resolve_data_dir(home: &Path) -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join(APP_NAME)
    } else {
        home.join(".local").join("share").join(APP_NAME)
    }
}

#[cfg(target_os = "macos")]
fn resolve_cache_dir(home: &Path) -> PathBuf {
    home.join("Library").join("Caches").join(BUNDLE_ID)
}

#[cfg(not(target_os = "macos"))]
fn resolve_cache_dir(home: &Path) -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join(APP_NAME)
    } else {
        home.join(".cache").join(APP_NAME)
    }
}

#[cfg(target_os = "macos")]
fn resolve_log_dir(home: &Path) -> PathBuf {
    home.join("Library").join("Logs").join(APP_NAME)
}

#[cfg(not(target_os = "macos"))]
fn resolve_log_dir(home: &Path) -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(xdg).join(APP_NAME).join("logs")
    } else {
        home.join(".local").join("share").join(APP_NAME).join("logs")
    }
}

// ---------------------------------------------------------------------------
// Time Machine exclusion (macOS only)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn exclude_from_time_machine(path: &Path) {
    use std::process::Command;
    match Command::new("tmutil")
        .args(["addexclusion", &path.to_string_lossy()])
        .output()
    {
        Ok(output) if output.status.success() => {
            info!("TM-excluded: {}", path.display());
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("tmutil addexclusion failed for {}: {}", path.display(), stderr.trim());
        }
        Err(e) => {
            warn!("failed to run tmutil for {}: {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_produces_valid_paths() {
        let paths = BisquePaths::resolve().expect("HOME should be set in tests");
        assert!(paths.config.to_string_lossy().contains("bisque-computer"));
        assert!(paths.data.to_string_lossy().contains("bisque-computer"));
        assert!(paths.projects.ends_with("projects"));
        assert!(paths.containers.ends_with("containers"));
        assert!(paths.vms.ends_with("vms"));
        assert!(paths.models.ends_with("models"));
        assert!(paths.db.ends_with("db"));
    }

    #[test]
    fn ensure_creates_directories() {
        let tmp = std::env::temp_dir().join(format!(
            "bisque_paths_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let paths = BisquePaths {
            config: tmp.join("config"),
            data: tmp.join("data"),
            projects: tmp.join("data/projects"),
            containers: tmp.join("data/containers"),
            vms: tmp.join("data/vms"),
            models: tmp.join("data/models"),
            db: tmp.join("data/db"),
            cache: tmp.join("cache"),
            logs: tmp.join("logs"),
        };

        paths.ensure().expect("ensure should succeed");

        assert!(paths.config.is_dir());
        assert!(paths.projects.is_dir());
        assert!(paths.containers.is_dir());
        assert!(paths.vms.is_dir());
        assert!(paths.models.is_dir());
        assert!(paths.db.is_dir());
        assert!(paths.cache.is_dir());
        assert!(paths.logs.is_dir());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
