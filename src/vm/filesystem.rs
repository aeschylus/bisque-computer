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

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Top-level configuration for VM filesystem isolation.
#[derive(Debug, Clone)]
pub struct FsIsolationConfig {
    pub vm_disk_path: PathBuf,
    pub drop_folder_host_path: PathBuf,
    pub drop_folder_vm_path: String,
}

impl FsIsolationConfig {
    pub fn from_data_dir(data_dir: &Path) -> Self {
        Self {
            vm_disk_path: data_dir.join("vms").join("lobster-vm.img"),
            drop_folder_host_path: data_dir.join("lobster-drop"),
            drop_folder_vm_path: "/mnt/lobster-drop".to_string(),
        }
    }

    pub fn drop_share(&self) -> VirtioFsShare {
        VirtioFsShare {
            host_path: self.drop_folder_host_path.clone(),
            vm_mount_point: self.drop_folder_vm_path.clone(),
            tag: VIRTIOFS_DROP_TAG.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VirtioFsShare {
    pub host_path: PathBuf,
    pub vm_mount_point: String,
    pub tag: String,
}

pub const VIRTIOFS_DROP_TAG: &str = "lobster-drop";

#[cfg(target_os = "macos")]
pub fn create_vm_disk(path: &Path, size_mb: u64) -> Result<()> {
    let size_arg = format!("{}M", size_mb);
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
        bail!("truncate failed (exit {}): {}", output.status, stderr.trim());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn create_vm_disk(_path: &Path, _size_mb: u64) -> Result<()> {
    bail!("create_vm_disk is not implemented on this platform (macOS only)")
}

#[cfg(target_os = "macos")]
pub fn format_vm_disk(path: &Path) -> Result<()> {
    let mkfs_candidates = [
        "/opt/homebrew/sbin/mkfs.ext4",
        "/usr/local/sbin/mkfs.ext4",
        "mkfs.ext4",
    ];
    let mkfs_bin = mkfs_candidates
        .iter()
        .find(|candidate| {
            if candidate.starts_with('/') {
                std::path::Path::new(candidate).exists()
            } else {
                true
            }
        })
        .copied()
        .unwrap_or("mkfs.ext4");
    let output = Command::new(mkfs_bin)
        .args(["-F", &path.to_string_lossy()])
        .output()
        .with_context(|| format!("spawn `{}` — install e2fsprogs: `brew install e2fsprogs`", mkfs_bin))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("`{}` failed (exit {}): {}", mkfs_bin, output.status, stderr.trim());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn format_vm_disk(_path: &Path) -> Result<()> {
    bail!("format_vm_disk is not implemented on this platform (macOS only)")
}

#[cfg(target_os = "macos")]
pub fn build_vfkit_virtiofs_args(share: &VirtioFsShare) -> Vec<String> {
    let device_spec = format!(
        "virtio-fs,sharedDir={},mountTag={}",
        share.host_path.display(),
        share.tag,
    );
    vec!["--device".to_string(), device_spec]
}

#[cfg(not(target_os = "macos"))]
pub fn build_vfkit_virtiofs_args(_share: &VirtioFsShare) -> Vec<String> {
    Vec::new()
}

#[cfg(target_os = "macos")]
pub fn verify_isolation(vm_serial_socket: &Path) -> Result<()> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    const SENTINEL_PATH: &str = "/host_sentinel_check";
    const DONE_MARKER: &str = "__DONE__";
    const TIMEOUT: Duration = Duration::from_secs(5);

    let mut stream = UnixStream::connect(vm_serial_socket)
        .with_context(|| format!("connect to VM serial socket: {}", vm_serial_socket.display()))?;
    stream.set_read_timeout(Some(TIMEOUT)).context("set read timeout")?;
    stream.set_write_timeout(Some(TIMEOUT)).context("set write timeout")?;

    let probe = format!("cat {sentinel} 2>&1; echo {done}\n",
        sentinel = SENTINEL_PATH, done = DONE_MARKER);
    stream.write_all(probe.as_bytes()).context("write probe command")?;

    let mut buf = [0u8; 4096];
    let mut response = String::new();
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                response.push_str(&String::from_utf8_lossy(&buf[..n]));
                if response.contains(DONE_MARKER) { break; }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                bail!("timed out waiting for VM serial response after {:?}", TIMEOUT)
            }
            Err(e) => return Err(e).context("read from VM serial socket"),
        }
    }

    if response.contains("host_sentinel_content") {
        bail!("filesystem isolation BREACH: VM can read host sentinel at {}.", SENTINEL_PATH);
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn verify_isolation(_vm_serial_socket: &Path) -> Result<()> {
    bail!("verify_isolation is not implemented on this platform (macOS only)")
}

pub fn ensure_drop_folder(config: &FsIsolationConfig) -> Result<()> {
    std::fs::create_dir_all(&config.drop_folder_host_path).with_context(|| {
        format!("create drop folder: {}", config.drop_folder_host_path.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn fs_isolation_config_from_data_dir() {
        let data = PathBuf::from("/tmp/bisque-data");
        let cfg = FsIsolationConfig::from_data_dir(&data);
        assert_eq!(cfg.vm_disk_path, PathBuf::from("/tmp/bisque-data/vms/lobster-vm.img"));
        assert_eq!(cfg.drop_folder_host_path, PathBuf::from("/tmp/bisque-data/lobster-drop"));
        assert_eq!(cfg.drop_folder_vm_path, "/mnt/lobster-drop");
    }

    #[test]
    fn drop_share_tag_is_canonical() {
        let data = PathBuf::from("/tmp/bisque-data");
        let cfg = FsIsolationConfig::from_data_dir(&data);
        let share = cfg.drop_share();
        assert_eq!(share.tag, VIRTIOFS_DROP_TAG);
    }

    #[test]
    fn ensure_drop_folder_creates_directory() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let tmp = std::env::temp_dir().join(format!("bisque_fs_test_{}", nanos));
        let cfg = FsIsolationConfig {
            vm_disk_path: tmp.join("vms/lobster-vm.img"),
            drop_folder_host_path: tmp.join("lobster-drop"),
            drop_folder_vm_path: "/mnt/lobster-drop".to_string(),
        };
        ensure_drop_folder(&cfg).expect("should create drop folder");
        assert!(cfg.drop_folder_host_path.is_dir());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn create_vm_disk_returns_err_on_non_macos() {
        let img = std::env::temp_dir().join("test.img");
        let result = create_vm_disk(&img, 64);
        assert!(result.is_err());
    }
}
