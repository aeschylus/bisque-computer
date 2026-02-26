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
//! ## Virtual Disk
//!
//! A fresh virtual disk is created at first launch and stored in the app's VM
//! directory. The VM sees it as `/dev/vda`. The disk is a sparse file so it
//! only consumes space proportional to what the VM writes.
//!
//! ## External Requirements (macOS)
//!
//! - `mkfs.ext4`: provided by the `e2fsprogs` package (`brew install e2fsprogs`).
//!   Required only when calling `format_vm_disk`. If unavailable, the function
//!   returns an error with an actionable message.
//! - `truncate` (BSD): ships with macOS. Used by `create_vm_disk`.
//! - `vfkit`: a vfkit binary must be on `$PATH` or its path passed explicitly
//!   when launching the VM. The arguments produced by `build_vfkit_virtiofs_args`
//!   target the `vfkit` CLI (`--device virtio-fs,...`).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// Top-level configuration for VM filesystem isolation.
///
/// Carries the three paths that define the isolation boundary:
/// - the VM's own virtual disk
/// - the host-side drop folder (the only shared directory)
/// - the in-VM mount point for that drop folder
#[derive(Debug, Clone)]
pub struct FsIsolationConfig {
    /// Absolute path to the raw disk image (`.img`) on the host.
    /// Created by `create_vm_disk` and formatted by `format_vm_disk`.
    pub vm_disk_path: PathBuf,

    /// Absolute path to the drop folder on the host.
    ///
    /// Defaults to `~/Library/Application Support/com.fullyparsed.bisque-computer/lobster-drop/`.
    /// Created at launch if it does not exist.
    pub drop_folder_host_path: PathBuf,

    /// Absolute path inside the VM where the drop folder is mounted.
    ///
    /// Defaults to `/mnt/lobster-drop`. The VM's init system must run:
    /// `mount -t virtiofs lobster-drop /mnt/lobster-drop`
    pub drop_folder_vm_path: String,
}

impl FsIsolationConfig {
    /// Construct a config with all paths resolved under the given `data_dir`
    /// (typically `BisquePaths::data`).
    ///
    /// - `vm_disk_path`          → `<data_dir>/vms/lobster-vm.img`
    /// - `drop_folder_host_path` → `<data_dir>/lobster-drop/`
    /// - `drop_folder_vm_path`   → `/mnt/lobster-drop`
    pub fn from_data_dir(data_dir: &Path) -> Self {
        Self {
            vm_disk_path: data_dir.join("vms").join("lobster-vm.img"),
            drop_folder_host_path: data_dir.join("lobster-drop"),
            drop_folder_vm_path: "/mnt/lobster-drop".to_string(),
        }
    }

    /// Build the corresponding `VirtioFsShare` from this config.
    pub fn drop_share(&self) -> VirtioFsShare {
        VirtioFsShare {
            host_path: self.drop_folder_host_path.clone(),
            vm_mount_point: self.drop_folder_vm_path.clone(),
            tag: VIRTIOFS_DROP_TAG.to_string(),
        }
    }
}

/// A virtio-fs directory share exposed to the VM.
///
/// One `VirtioFsShare` corresponds to one `--device virtio-fs,...` argument
/// passed to vfkit (or equivalently, one `VZVirtioFileSystemDeviceConfiguration`
/// in `Virtualization.framework`).
#[derive(Debug, Clone)]
pub struct VirtioFsShare {
    /// Absolute path to the directory on the host to share.
    pub host_path: PathBuf,

    /// Absolute path inside the VM where the share is mounted.
    /// Informational — the VM's init system is responsible for the `mount` call.
    pub vm_mount_point: String,

    /// virtiofs tag string. The VM uses this tag in the mount command:
    /// `mount -t virtiofs <tag> <vm_mount_point>`.
    pub tag: String,
}

/// The canonical virtiofs tag for the Lobster Drop folder.
pub const VIRTIOFS_DROP_TAG: &str = "lobster-drop";

// ---------------------------------------------------------------------------
// Disk management (macOS)
// ---------------------------------------------------------------------------

/// Create a blank sparse raw disk image at `path` with size `size_mb` megabytes.
///
/// Uses `truncate -s <size>` (BSD/macOS) to create a sparse file that only
/// consumes real disk blocks when written. On a 10 GB image, the file is
/// created instantly and occupies ~0 bytes until the VM writes data.
///
/// # Errors
///
/// Returns an error if:
/// - `path` cannot be created (permissions, parent does not exist)
/// - `truncate` is not available or exits with a non-zero status
///
/// # Platform
///
/// gated on `#[cfg(target_os = "macos")]`; see the Linux stub below.
#[cfg(target_os = "macos")]
pub fn create_vm_disk(path: &Path, size_mb: u64) -> Result<()> {
    let size_arg = format!("{}M", size_mb);

    // Ensure the parent directory exists before calling truncate.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create parent directory: {}", parent.display()))?;
    }

    let output = Command::new("truncate")
        .args(["-s", &size_arg, &path.to_string_lossy()])
        .output()
        .context("spawn `truncate` — is it on $PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "truncate failed (exit {}): {}",
            output.status,
            stderr.trim()
        );
    }

    Ok(())
}

/// Linux stub — returns `Err` with a "not implemented" message.
#[cfg(not(target_os = "macos"))]
pub fn create_vm_disk(_path: &Path, _size_mb: u64) -> Result<()> {
    bail!("create_vm_disk is not implemented on this platform (macOS only)")
}

/// Format an existing raw disk image as ext4.
///
/// Calls `mkfs.ext4 -F <path>`. The `-F` flag forces formatting even when
/// `mkfs.ext4` cannot confirm that `path` is a block device.
///
/// # Requirements
///
/// `mkfs.ext4` must be on `$PATH`. On macOS, install via Homebrew:
///
/// ```sh
/// brew install e2fsprogs
/// export PATH="/opt/homebrew/sbin:$PATH"   # add to your shell profile
/// ```
///
/// The formula installs the binary as `mkfs.ext4` (symlinked from `mke2fs`).
///
/// # Errors
///
/// Returns an error if:
/// - `mkfs.ext4` is not on `$PATH`
/// - `path` does not exist or cannot be opened
/// - `mkfs.ext4` exits with a non-zero status
///
/// # Platform
///
/// gated on `#[cfg(target_os = "macos")]`; see the Linux stub below.
#[cfg(target_os = "macos")]
pub fn format_vm_disk(path: &Path) -> Result<()> {
    // Prefer the Homebrew-installed binary; fall back to anything on $PATH.
    let mkfs_candidates = [
        "/opt/homebrew/sbin/mkfs.ext4",
        "/usr/local/sbin/mkfs.ext4",
        "mkfs.ext4",
    ];

    let mkfs_bin = mkfs_candidates
        .iter()
        .find(|candidate| {
            // Quick existence check for absolute paths; trust $PATH for bare names.
            if candidate.starts_with('/') {
                std::path::Path::new(candidate).exists()
            } else {
                true // let Command handle PATH resolution
            }
        })
        .copied()
        .unwrap_or("mkfs.ext4");

    let output = Command::new(mkfs_bin)
        .args(["-F", &path.to_string_lossy()])
        .output()
        .with_context(|| {
            format!(
                "spawn `{}` — install e2fsprogs: `brew install e2fsprogs` \
                 then add `/opt/homebrew/sbin` to $PATH",
                mkfs_bin
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "`{}` failed (exit {}): {}\n\
             Hint: install e2fsprogs via `brew install e2fsprogs`",
            mkfs_bin,
            output.status,
            stderr.trim()
        );
    }

    Ok(())
}

/// Linux stub — returns `Err` with a "not implemented" message.
#[cfg(not(target_os = "macos"))]
pub fn format_vm_disk(_path: &Path) -> Result<()> {
    bail!("format_vm_disk is not implemented on this platform (macOS only)")
}

// ---------------------------------------------------------------------------
// vfkit argument generation (macOS)
// ---------------------------------------------------------------------------

/// Generate the `--device virtio-fs,...` command-line arguments for vfkit.
///
/// vfkit accepts the following form for virtiofs shares:
///
/// ```text
/// --device virtio-fs,sharedDir=<host_path>,mountTag=<tag>
/// ```
///
/// This function returns a `Vec<String>` suitable for passing directly to
/// `std::process::Command::args`.
///
/// # Example
///
/// ```rust
/// # use bisque_computer::vm::filesystem::{VirtioFsShare, build_vfkit_virtiofs_args};
/// # use std::path::PathBuf;
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
///
/// # Platform
///
/// gated on `#[cfg(target_os = "macos")]`; see the Linux stub below.
#[cfg(target_os = "macos")]
pub fn build_vfkit_virtiofs_args(share: &VirtioFsShare) -> Vec<String> {
    let device_spec = format!(
        "virtio-fs,sharedDir={},mountTag={}",
        share.host_path.display(),
        share.tag,
    );
    vec!["--device".to_string(), device_spec]
}

/// Linux stub — returns an empty `Vec`.
#[cfg(not(target_os = "macos"))]
pub fn build_vfkit_virtiofs_args(_share: &VirtioFsShare) -> Vec<String> {
    Vec::new()
}

// ---------------------------------------------------------------------------
// Isolation smoke test (macOS)
// ---------------------------------------------------------------------------

/// Verify filesystem isolation by sending a test command over the VM's serial socket.
///
/// Writes a shell command to the serial console that attempts to read a
/// host-only sentinel path (`/etc/passwd` as seen from the VM perspective —
/// which, under isolation, does NOT map to the host `/etc/passwd`). The
/// function reads back the response and checks that the VM cannot access
/// host-exclusive paths.
///
/// # Protocol
///
/// 1. Open `vm_serial_socket` as a Unix domain stream socket.
/// 2. Send: `ls /host_sentinel_check 2>&1; echo __DONE__\n`
/// 3. Read until `__DONE__` appears in the output (timeout: 5 s).
/// 4. If the sentinel file content appears in the output, fail — isolation
///    has been breached.
///
/// This is a lightweight smoke test; it does not replace a full security
/// audit. The VM init system must have the serial console attached at the
/// path passed in `vm_serial_socket`.
///
/// # Errors
///
/// Returns `Ok(())` if the VM correctly denies access to the host-only path.
/// Returns `Err` if:
/// - The socket cannot be opened or written to
/// - The response times out
/// - Host filesystem content is visible inside the VM
///
/// # Platform
///
/// gated on `#[cfg(target_os = "macos")]`; see the Linux stub below.
#[cfg(target_os = "macos")]
pub fn verify_isolation(vm_serial_socket: &Path) -> Result<()> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    // A sentinel path that should NOT exist inside the VM but would be readable
    // if host mounts leaked through. We use /host_sentinel_check — a path that
    // exists only on the host (written during test setup) and must be absent
    // inside the isolated VM.
    const SENTINEL_PATH: &str = "/host_sentinel_check";
    const DONE_MARKER: &str = "__DONE__";
    const TIMEOUT: Duration = Duration::from_secs(5);

    let mut stream = UnixStream::connect(vm_serial_socket).with_context(|| {
        format!(
            "connect to VM serial socket: {}",
            vm_serial_socket.display()
        )
    })?;

    stream
        .set_read_timeout(Some(TIMEOUT))
        .context("set read timeout on serial socket")?;
    stream
        .set_write_timeout(Some(TIMEOUT))
        .context("set write timeout on serial socket")?;

    // Send a probe command that will echo DONE when finished.
    let probe = format!(
        "cat {sentinel} 2>&1; echo {done}\n",
        sentinel = SENTINEL_PATH,
        done = DONE_MARKER
    );
    stream
        .write_all(probe.as_bytes())
        .context("write probe command to serial socket")?;

    // Accumulate response until DONE marker or timeout.
    let mut buf = [0u8; 4096];
    let mut response = String::new();

    loop {
        match stream.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => {
                response.push_str(&String::from_utf8_lossy(&buf[..n]));
                if response.contains(DONE_MARKER) {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                bail!(
                    "timed out waiting for VM serial response after {:?}",
                    TIMEOUT
                )
            }
            Err(e) => return Err(e).context("read from VM serial socket"),
        }
    }

    // The VM must NOT return the sentinel file's contents. A "No such file"
    // error from the VM shell is the expected (safe) outcome.
    if response.contains("host_sentinel_content") {
        bail!(
            "filesystem isolation BREACH: VM can read host sentinel at {}.\n\
             Response: {}",
            SENTINEL_PATH,
            response.trim()
        );
    }

    Ok(())
}

/// Linux stub — returns `Err` with a "not implemented" message.
#[cfg(not(target_os = "macos"))]
pub fn verify_isolation(_vm_serial_socket: &Path) -> Result<()> {
    bail!("verify_isolation is not implemented on this platform (macOS only)")
}

// ---------------------------------------------------------------------------
// Host-side setup helpers
// ---------------------------------------------------------------------------

/// Ensure the drop folder exists on the host, creating it if needed.
///
/// This is a pure filesystem operation with no VM interaction. Call it at
/// application launch before starting the VM.
pub fn ensure_drop_folder(config: &FsIsolationConfig) -> Result<()> {
    std::fs::create_dir_all(&config.drop_folder_host_path).with_context(|| {
        format!(
            "create drop folder: {}",
            config.drop_folder_host_path.display()
        )
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("bisque_fs_test_{}", nanos))
    }

    #[test]
    fn fs_isolation_config_from_data_dir() {
        let data = PathBuf::from("/tmp/bisque-data");
        let cfg = FsIsolationConfig::from_data_dir(&data);

        assert_eq!(
            cfg.vm_disk_path,
            PathBuf::from("/tmp/bisque-data/vms/lobster-vm.img")
        );
        assert_eq!(
            cfg.drop_folder_host_path,
            PathBuf::from("/tmp/bisque-data/lobster-drop")
        );
        assert_eq!(cfg.drop_folder_vm_path, "/mnt/lobster-drop");
    }

    #[test]
    fn drop_share_tag_is_canonical() {
        let data = PathBuf::from("/tmp/bisque-data");
        let cfg = FsIsolationConfig::from_data_dir(&data);
        let share = cfg.drop_share();
        assert_eq!(share.tag, VIRTIOFS_DROP_TAG);
        assert_eq!(share.vm_mount_point, "/mnt/lobster-drop");
    }

    #[test]
    fn ensure_drop_folder_creates_directory() {
        let tmp = tmp_dir();
        let cfg = FsIsolationConfig {
            vm_disk_path: tmp.join("vms/lobster-vm.img"),
            drop_folder_host_path: tmp.join("lobster-drop"),
            drop_folder_vm_path: "/mnt/lobster-drop".to_string(),
        };

        ensure_drop_folder(&cfg).expect("should create drop folder");
        assert!(cfg.drop_folder_host_path.is_dir());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(target_os = "macos")]
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

    #[cfg(target_os = "macos")]
    #[test]
    fn create_vm_disk_produces_file() {
        let tmp = tmp_dir();
        std::fs::create_dir_all(&tmp).unwrap();
        let img = tmp.join("test.img");

        create_vm_disk(&img, 64).expect("create_vm_disk should succeed");
        assert!(img.exists(), "disk image file should exist");

        // Sparse file — size on disk may be 0 but file size should be 64 MiB.
        let meta = std::fs::metadata(&img).unwrap();
        assert_eq!(meta.len(), 64 * 1024 * 1024);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn create_vm_disk_returns_err_on_non_macos() {
        let tmp = tmp_dir();
        let img = tmp.join("test.img");
        let result = create_vm_disk(&img, 64);
        assert!(result.is_err());
    }
}
