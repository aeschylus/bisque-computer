/// VM management module for bisque-computer.
///
/// Provides VM lifecycle management (spawn, monitor, stop) via vfkit on macOS,
/// with a Linux stub returning `Err` so the crate compiles on all platforms.
///
/// ## Sub-modules
///
/// - [`lifecycle`] — spawn/stop/health-check a Linux microVM via vfkit REST API
///
/// ## Platform gating
///
/// All vfkit code is gated with `#[cfg(target_os = "macos")]`.
/// On Linux, stub functions return `Err` with a clear message so the crate
/// still compiles on CI and Linux development machines.

mod lifecycle;

/// Configuration for a Linux microVM launched via vfkit.
///
/// All path fields must point to existing files on the host at the time
/// [`spawn_vm`] is called — the function validates them eagerly before
/// attempting to start the child process.
#[derive(Debug, Clone)]
pub struct VmConfig {
    /// Path to the Linux kernel image (e.g. `vmlinuz`).
    pub kernel_path: std::path::PathBuf,

    /// Path to the initial ramdisk image (e.g. `initrd.img`).
    pub initrd_path: std::path::PathBuf,

    /// Path to the raw disk image (raw or qcow2, passed as virtio-blk).
    pub disk_path: std::path::PathBuf,

    /// Guest RAM in megabytes (e.g. `2048` for 2 GiB).
    pub memory_mb: u32,

    /// Number of virtual CPUs to expose to the guest.
    pub cpu_count: u32,

    /// File path where vfkit writes the guest serial console output.
    pub serial_log_path: std::path::PathBuf,

    /// TCP port for the vfkit REST management API (`GET/PUT /vm/state`).
    pub rest_port: u16,
}

// ---------------------------------------------------------------------------
// Re-exports from lifecycle sub-module
// ---------------------------------------------------------------------------

pub use lifecycle::{VmHandle, VmState, spawn_vm};
