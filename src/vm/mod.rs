//! VM management module for bisque-computer.
//!
//! Provides filesystem isolation, virtual disk management, virtio-fs share
//! configuration, the drop folder server for file ingestion, and the remote
//! communication channel for the Lobster VM sandbox.

use std::path::PathBuf;

pub mod claude_isolation;
pub mod drop_server;
pub mod filesystem;
pub mod lifecycle;
pub mod provisioning;
pub mod remote_channel;

// ---------------------------------------------------------------------------
// Shared types used across submodules
// ---------------------------------------------------------------------------

/// Top-level configuration for spawning a Lobster VM.
///
/// Passed to [`lifecycle::spawn_vm`] to build and launch the vfkit command.
/// All paths must exist on disk before calling `spawn_vm`.
#[derive(Debug, Clone)]
pub struct VmConfig {
    /// Path to the Linux kernel image (e.g. `vmlinuz`).
    pub kernel_path: PathBuf,

    /// Path to the initial ramdisk image (e.g. `initrd.img`).
    pub initrd_path: PathBuf,

    /// Path to the raw disk image (see [`filesystem::create_vm_disk`]).
    pub disk_path: PathBuf,

    /// Path where vfkit writes the guest serial console output.
    pub serial_log_path: PathBuf,

    /// TCP port for the vfkit REST management API (`GET/PUT /vm/state`).
    pub rest_port: u16,

    /// Number of virtual CPUs to give the VM.
    pub cpu_count: u32,

    /// Memory allocation in megabytes.
    pub memory_mb: u32,
}

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use lifecycle::{VmHandle, VmState, spawn_vm};
