/// VM lifecycle management via vfkit (macOS Virtualization.framework wrapper).
///
/// This module is gated to macOS only. On Linux a stub is provided that returns
/// an explanatory error so the rest of the codebase can compile unconditionally.
///
/// ## Architecture
///
/// ```text
/// spawn_vm(VmConfig)
///     └─► tokio::process::Command  →  vfkit child process
///             └─► VmHandle { child, rest_port, state }
///                     ├─► health_check task  (polls GET /vm/state every 5 s)
///                     └─► stop()             (PUT /vm/state {"state":"Stop"})
/// ```

// ---------------------------------------------------------------------------
// macOS implementation
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod imp {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use anyhow::{Context, bail};
    use tokio::process::{Child, Command};
    use tokio::sync::RwLock;
    use tracing::{debug, error, info, warn};

    use crate::vm::VmConfig;

    /// Observed lifecycle state of the guest VM.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum VmState {
        /// The vfkit process is running and the guest appears healthy.
        Running,
        /// A stop was requested and we are waiting for process exit.
        Stopping,
        /// The vfkit process has exited (cleanly or due to an error).
        Stopped,
        /// The health-check received an error response from the REST API.
        Failed(String),
    }

    /// A live handle to a running VM.
    ///
    /// Dropping this value does NOT stop the child process — call
    /// `VmHandle::stop` explicitly for a graceful shutdown.
    pub struct VmHandle {
        pub(crate) child: Arc<RwLock<Option<Child>>>,
        pub(crate) rest_port: u16,
        pub(crate) state: Arc<RwLock<VmState>>,
    }

    impl VmHandle {
        /// Returns the current lifecycle state of the VM.
        pub async fn state(&self) -> VmState {
            self.state.read().await.clone()
        }

        /// Returns `true` when the child process is still alive and healthy.
        pub async fn is_running(&self) -> bool {
            matches!(self.state().await, VmState::Running)
        }

        /// Gracefully stop the VM via the vfkit REST API, then wait for the
        /// child process to exit. If the REST call fails the child is killed.
        pub async fn stop(&self) -> anyhow::Result<()> {
            {
                let mut state = self.state.write().await;
                *state = VmState::Stopping;
            }

            let url = format!("http://localhost:{}/vm/state", self.rest_port);
            let client = reqwest::Client::new();

            let stop_result = client
                .put(&url)
                .json(&serde_json::json!({"state": "Stop"}))
                .timeout(Duration::from_secs(10))
                .send()
                .await;

            match stop_result {
                Ok(resp) => {
                    info!(
                        port = self.rest_port,
                        status = resp.status().as_u16(),
                        "VM stop request sent"
                    );
                }
                Err(e) => {
                    warn!(
                        port = self.rest_port,
                        error = %e,
                        "REST stop failed — killing child process"
                    );
                    if let Some(child) = self.child.write().await.as_mut() {
                        let _ = child.kill().await;
                    }
                }
            }

            // Wait for the process to exit (up to 30 s).
            let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
            loop {
                {
                    let mut guard = self.child.write().await;
                    if let Some(child) = guard.as_mut() {
                        match child.try_wait() {
                            Ok(Some(status)) => {
                                info!(exit_status = ?status, "VM process exited");
                                *self.state.write().await = VmState::Stopped;
                                return Ok(());
                            }
                            Ok(None) => {}
                            Err(e) => {
                                error!(error = %e, "Error waiting for VM process");
                                *self.state.write().await = VmState::Stopped;
                                return Err(e.into());
                            }
                        }
                    } else {
                        *self.state.write().await = VmState::Stopped;
                        return Ok(());
                    }
                }

                if tokio::time::Instant::now() >= deadline {
                    warn!("VM did not exit within 30 s — killing");
                    if let Some(child) = self.child.write().await.as_mut() {
                        let _ = child.kill().await;
                    }
                    *self.state.write().await = VmState::Stopped;
                    return Ok(());
                }

                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    /// Resolve the path to the `vfkit` binary, returning an error if it is not
    /// found in any standard location.
    fn find_vfkit() -> anyhow::Result<PathBuf> {
        // Also honour PATH first.
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in path_var.split(':') {
                let candidate = PathBuf::from(dir).join("vfkit");
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }

        // Common install locations (Homebrew on Intel and Apple Silicon).
        let candidates: &[&str] = &[
            "/opt/homebrew/bin/vfkit",
            "/usr/local/bin/vfkit",
        ];

        for &path in candidates {
            if PathBuf::from(path).exists() {
                return Ok(PathBuf::from(path));
            }
        }

        bail!(
            "vfkit binary not found. Install it with: brew install vfkit\n\
             Or set PATH to include the directory containing vfkit."
        )
    }

    /// Build the `vfkit` command from the given `VmConfig`.
    fn build_vfkit_command(vfkit: &PathBuf, config: &VmConfig) -> Command {
        let mut cmd = Command::new(vfkit);

        // Bootloader.
        cmd.arg("--bootloader").arg(format!(
            "linux,kernel={},initrd={},cmdline=console=hvc0 root=/dev/vda rw",
            config.kernel_path.display(),
            config.initrd_path.display()
        ));

        // Resources.
        cmd.arg("--cpus").arg(config.cpu_count.to_string());
        cmd.arg("--memory").arg(config.memory_mb.to_string());

        // Disk (virtio-blk).
        cmd.arg("--device").arg(format!(
            "virtio-blk,path={}",
            config.disk_path.display()
        ));

        // Serial console to log file.
        cmd.arg("--device").arg(format!(
            "virtio-serial,logFilePath={}",
            config.serial_log_path.display()
        ));

        // REST API for state management.
        cmd.arg("--restful-uri")
            .arg(format!("tcp://localhost:{}", config.rest_port));

        // Entropy device (required for kernel RNG).
        cmd.arg("--device").arg("virtio-rng");

        cmd
    }

    /// Spawn a background tokio task that polls `GET /vm/state` every 5 seconds
    /// and updates the shared `VmState`.
    fn spawn_health_check(
        rest_port: u16,
        state: Arc<RwLock<VmState>>,
        child: Arc<RwLock<Option<Child>>>,
    ) {
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client build failed");

            let url = format!("http://localhost:{}/vm/state", rest_port);

            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;

                // If we already know the VM has stopped, exit the loop.
                {
                    let current = state.read().await.clone();
                    if matches!(current, VmState::Stopped | VmState::Stopping) {
                        debug!(port = rest_port, "health-check task exiting (VM stopped)");
                        return;
                    }
                }

                // Check if child process is still alive.
                {
                    let mut guard = child.write().await;
                    if let Some(child_proc) = guard.as_mut() {
                        match child_proc.try_wait() {
                            Ok(Some(exit_status)) => {
                                info!(
                                    port = rest_port,
                                    exit_status = ?exit_status,
                                    "VM process exited unexpectedly"
                                );
                                *state.write().await = VmState::Stopped;
                                return;
                            }
                            Ok(None) => {}
                            Err(e) => {
                                error!(port = rest_port, error = %e, "try_wait error in health-check");
                            }
                        }
                    }
                }

                // Poll the REST API.
                match client.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let body = resp.text().await.unwrap_or_default();
                        debug!(port = rest_port, state = %body, "VM health-check OK");
                        *state.write().await = VmState::Running;
                    }
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        warn!(port = rest_port, http_status = status, "VM health-check non-2xx");
                        *state.write().await = VmState::Failed(format!("HTTP {}", status));
                    }
                    Err(e) => {
                        warn!(port = rest_port, error = %e, "VM health-check failed");
                        // Don't immediately mark as Failed — the guest may still be
                        // booting.  Keep Running until we confirm exit.
                    }
                }
            }
        });
    }

    /// Spawn a Linux microVM using vfkit and return a `VmHandle`.
    ///
    /// Returns `Err` if:
    /// - `vfkit` binary is not found on the system
    /// - Any required path in `config` does not exist
    /// - The child process fails to start
    pub async fn spawn_vm(config: VmConfig) -> anyhow::Result<VmHandle> {
        // Validate paths eagerly so callers get a clear error.
        if !config.kernel_path.exists() {
            bail!("kernel_path does not exist: {}", config.kernel_path.display());
        }
        if !config.initrd_path.exists() {
            bail!("initrd_path does not exist: {}", config.initrd_path.display());
        }
        if !config.disk_path.exists() {
            bail!("disk_path does not exist: {}", config.disk_path.display());
        }

        let vfkit_path = find_vfkit()?;
        info!(path = %vfkit_path.display(), "Found vfkit binary");

        let mut cmd = build_vfkit_command(&vfkit_path, &config);

        // Redirect child stdio so it does not pollute our terminal.
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());

        let child = cmd
            .spawn()
            .context("Failed to spawn vfkit process")?;

        info!(
            port = config.rest_port,
            kernel = %config.kernel_path.display(),
            memory_mb = config.memory_mb,
            cpus = config.cpu_count,
            "VM spawned"
        );

        let rest_port = config.rest_port;
        let child_arc = Arc::new(RwLock::new(Some(child)));
        let state_arc = Arc::new(RwLock::new(VmState::Running));

        spawn_health_check(rest_port, state_arc.clone(), child_arc.clone());

        Ok(VmHandle {
            child: child_arc,
            rest_port,
            state: state_arc,
        })
    }
}

// ---------------------------------------------------------------------------
// Linux / non-macOS stub
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::vm::VmConfig;

    /// Placeholder state type for non-macOS builds.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum VmState {
        /// Always `Stopped` on non-macOS platforms.
        Stopped,
    }

    /// Placeholder handle — always errors on non-macOS platforms.
    pub struct VmHandle {
        pub(crate) rest_port: u16,
    }

    impl VmHandle {
        pub async fn state(&self) -> VmState {
            VmState::Stopped
        }

        pub async fn is_running(&self) -> bool {
            false
        }

        pub async fn stop(&self) -> anyhow::Result<()> {
            Err(anyhow::anyhow!("VM lifecycle requires macOS"))
        }
    }

    /// On non-macOS platforms this always returns an error.
    pub async fn spawn_vm(_config: VmConfig) -> anyhow::Result<VmHandle> {
        Err(anyhow::anyhow!(
            "VM lifecycle requires macOS (Virtualization.framework via vfkit)"
        ))
    }
}

// ---------------------------------------------------------------------------
// Re-export the platform-appropriate symbols
// ---------------------------------------------------------------------------

pub use imp::{VmHandle, VmState, spawn_vm};
