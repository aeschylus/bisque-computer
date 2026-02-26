//! Filesystem isolation and virtio-fs drop folder share for the Lobster VM.
//!
//! ## Design
//!
//! The VM runs with complete filesystem isolation from the host. No host paths
//! are mounted by default. The sole exception is the "Lobster Drop" folder,
//! which is exposed as a read-write virtio-fs share inside the VM.
//!
//! ```text
//! Host:  ~/Library/Application Support/com.fullyparsed.bisque-computer/lobster-drop/
//!                           |
//!                    virtio-fs (tag: "lobster-drop")
//!                           |
//! VM:    /mnt/lobster-drop/
//! ```
//!
//! ## External Requirements (macOS)
//!
//! - `vfkit`: must be on `$PATH`. The arguments produced by
//!   `build_vfkit_virtiofs_args` target the `vfkit` CLI
//!   (`--device virtio-fs,...`).

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// A virtio-fs directory share exposed to the VM.
///
/// One `VirtioFsShare` corresponds to one `--device virtio-fs,...` argument
/// passed to vfkit. The `tag` is the virtiofs tag string used by the VM's
/// mount command: `mount -t virtiofs <tag> <vm_mount_point>`.
#[derive(Debug, Clone)]
pub struct VirtioFsShare {
    /// Absolute path to the directory on the host to share.
    pub host_path: PathBuf,

    /// Absolute path inside the VM where the share is mounted.
    /// Informational only — the VM's init system is responsible for the
    /// actual `mount` call using `tag`.
    pub vm_mount_point: String,

    /// The virtiofs tag string. The VM uses this tag in the mount command:
    /// `mount -t virtiofs <tag> <vm_mount_point>`.
    pub tag: String,
}

/// The canonical virtiofs tag for the Lobster Drop folder.
pub const VIRTIOFS_DROP_TAG: &str = "lobster-drop";

// ---------------------------------------------------------------------------
// vfkit argument generation
// ---------------------------------------------------------------------------

/// Generate the `--device virtio-fs,...` command-line arguments for vfkit.
///
/// vfkit accepts the following form for virtiofs shares:
///
/// ```text
/// --device virtio-fs,sharedDir=<host_path>,mountTag=<tag>
/// ```
///
/// Returns a `Vec<String>` suitable for passing directly to
/// `std::process::Command::args` or `tokio::process::Command::args`.
///
/// # Example
///
/// ```rust
/// # use std::path::PathBuf;
/// # use bisque_computer::vm::filesystem::{VirtioFsShare, build_vfkit_virtiofs_args};
/// let share = VirtioFsShare {
///     host_path: PathBuf::from("/Users/drew/lobster-drop"),
///     vm_mount_point: "/mnt/lobster-drop".to_string(),
///     tag: "lobster-drop".to_string(),
/// };
/// let args = build_vfkit_virtiofs_args(&share);
/// assert_eq!(args, vec![
///     "--device".to_string(),
///     "virtio-fs,sharedDir=/Users/drew/lobster-drop,mountTag=lobster-drop".to_string(),
/// ]);
/// ```
pub fn build_vfkit_virtiofs_args(share: &VirtioFsShare) -> Vec<String> {
    let device_spec = format!(
        "virtio-fs,sharedDir={},mountTag={}",
        share.host_path.display(),
        share.tag,
    );
    vec!["--device".to_string(), device_spec]
}

// ---------------------------------------------------------------------------
// Host-side setup helpers
// ---------------------------------------------------------------------------

/// Ensure the drop folder exists on the host, creating it if needed.
///
/// This is a pure filesystem operation with no VM interaction. Call it at
/// application launch before starting the VM.
pub fn ensure_drop_folder(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .map_err(|e| anyhow::anyhow!("create drop folder {}: {}", path.display(), e))
}

// ---------------------------------------------------------------------------
// Virtual disk management (macOS only)
// ---------------------------------------------------------------------------

/// Create a blank sparse raw disk image at `path` with size `size_mb` megabytes.
///
/// Uses `truncate -s <size>` (BSD/macOS) to create a sparse file. The file
/// only consumes real disk blocks when written by the VM.
///
/// # Platform
///
/// This function is only meaningful on macOS. On other platforms it returns
/// `Err`.
#[cfg(target_os = "macos")]
pub fn create_vm_disk(path: &Path, size_mb: u64) -> Result<()> {
    use std::process::Command;

    let size_arg = format!("{}M", size_mb);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("create parent directory {}: {}", parent.display(), e))?;
    }

    let output = Command::new("truncate")
        .args(["-s", &size_arg, &path.to_string_lossy()])
        .output()
        .map_err(|e| anyhow::anyhow!("spawn `truncate`: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("truncate failed (exit {}): {}", output.status, stderr.trim());
    }

    Ok(())
}

/// Linux stub — returns `Err` on non-macOS platforms.
#[cfg(not(target_os = "macos"))]
pub fn create_vm_disk(_path: &Path, _size_mb: u64) -> Result<()> {
    bail!("create_vm_disk is only supported on macOS")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_vfkit_virtiofs_args_format() {
        let share = VirtioFsShare {
            host_path: PathBuf::from("/Users/drew/lobster-drop"),
            vm_mount_point: "/mnt/lobster-drop".to_string(),
            tag: "lobster-drop".to_string(),
        };

        let args = build_vfkit_virtiofs_args(&share);

        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "--device");
        assert!(args[1].starts_with("virtio-fs,sharedDir="));
        assert!(args[1].contains("mountTag=lobster-drop"));
        assert!(args[1].contains("/Users/drew/lobster-drop"));
    }

    #[test]
    fn build_vfkit_virtiofs_args_full_format() {
        let share = VirtioFsShare {
            host_path: PathBuf::from("/Users/drew/lobster-drop"),
            vm_mount_point: "/mnt/lobster-drop".to_string(),
            tag: "lobster-drop".to_string(),
        };

        let args = build_vfkit_virtiofs_args(&share);
        let expected = "virtio-fs,sharedDir=/Users/drew/lobster-drop,mountTag=lobster-drop";

        assert_eq!(args, vec!["--device".to_string(), expected.to_string()]);
    }

    #[test]
    fn create_vm_disk_returns_err_on_non_macos() {
        #[cfg(not(target_os = "macos"))]
        {
            let tmp = std::env::temp_dir().join("bisque_test_disk.img");
            let result = create_vm_disk(&tmp, 64);
            assert!(result.is_err());
        }
        // On macOS this test is skipped (the function works there).
        #[cfg(target_os = "macos")]
        {
            // No-op on macOS: covered by a separate test that actually
            // creates the file.
        }
    }
}
