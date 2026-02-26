//! Lobster instance provisioning inside a QEMU/vfkit VM.
//!
//! After a VM boots with sshd running, [`provision_lobster`] copies the
//! Lobster source tree into the guest, installs Node.js/Python dependencies,
//! installs Claude Code, and starts the Lobster MCP server in headless mode.
//!
//! All network I/O uses `tokio::process::Command` (SSH/SCP) so the caller can
//! stay async without blocking the event loop.
//!
//! # Platform gating
//!
//! The full implementation is compiled only on macOS (`#[cfg(target_os = "macos")]`).
//! On Linux a minimal stub is provided so the crate still compiles on CI and
//! Linux development machines — the stub returns `Err` with a clear message.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Public types (available on all platforms)
// ---------------------------------------------------------------------------

/// Configuration for provisioning a Lobster instance inside a VM.
///
/// All fields are kept in a plain struct so callers can construct them
/// declaratively; there is no hidden mutable state.
#[derive(Debug, Clone)]
pub struct ProvisionConfig {
    /// Path on the *host* to the Lobster source tree that will be copied into
    /// the VM at `/opt/lobster/`.
    pub lobster_source_path: PathBuf,

    /// Path on the host to the virtio-serial socket exposed by QEMU
    /// (`-chardev socket,path=…,server=on,wait=off -device virtio-serial …`).
    /// Reserved for future use; current implementation communicates via SSH.
    pub vm_serial_socket: PathBuf,

    /// Host-side port forwarded to the guest's SSH daemon (22).
    /// Typically `2222` when the VM is started with
    /// `-net user,hostfwd=tcp::2222-:22`.
    pub ssh_port: u16,
}

// ---------------------------------------------------------------------------
// macOS implementation
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod imp {
    use super::ProvisionConfig;
    use anyhow::{bail, Context, Result};
    use std::net::TcpStream;
    use std::time::{Duration, Instant};
    use tokio::process::Command;

    // -----------------------------------------------------------------------
    // Constants
    // -----------------------------------------------------------------------

    /// Remote user that sshd accepts inside the Alpine/Debian guest.
    const SSH_USER: &str = "root";

    /// Where Lobster lives inside the VM.
    const REMOTE_LOBSTER_DIR: &str = "/opt/lobster";

    /// SSH options shared by every ssh/scp invocation.
    ///
    /// * `StrictHostKeyChecking=no` — VM images are ephemeral; host keys
    ///   change on every fresh image boot.
    /// * `UserKnownHostsFile=/dev/null` — don't pollute the host's known_hosts.
    /// * `LogLevel=ERROR` — suppress banner noise from Alpine sshd.
    /// * `BatchMode=yes` — fail immediately if a password prompt would appear
    ///   (we rely on key-based auth or a password-less root account in the VM
    ///   image).
    const SSH_OPTS: &[&str] = &[
        "-o", "StrictHostKeyChecking=no",
        "-o", "UserKnownHostsFile=/dev/null",
        "-o", "LogLevel=ERROR",
        "-o", "BatchMode=yes",
        "-o", "ConnectTimeout=5",
    ];

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Wait until the VM's SSH port accepts TCP connections.
    ///
    /// Polls every 500 ms until `timeout_secs` elapses.  Returns `Ok(())` as
    /// soon as a TCP handshake succeeds.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the port is still unreachable after `timeout_secs`.
    pub async fn wait_for_vm_ready(ssh_port: u16, timeout_secs: u64) -> Result<()> {
        let addr = format!("127.0.0.1:{ssh_port}");
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);

        loop {
            // TcpStream::connect is blocking; run it on the blocking thread pool
            // so we don't stall the Tokio scheduler.
            let addr_clone = addr.clone();
            let connected = tokio::task::spawn_blocking(move || {
                TcpStream::connect_timeout(
                    &addr_clone.parse().expect("addr is valid"),
                    Duration::from_secs(2),
                )
                .is_ok()
            })
            .await
            .context("spawn_blocking for TCP probe failed")?;

            if connected {
                return Ok(());
            }

            if Instant::now() >= deadline {
                bail!(
                    "VM SSH port {ssh_port} did not become reachable within {timeout_secs}s"
                );
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Provision a fresh Lobster instance inside a running VM.
    ///
    /// Steps (idempotent):
    ///
    /// 1. Copy Lobster source tree into the VM via `scp -r`.
    /// 2. Install Node.js + Python inside the VM (`apk add` / `apt-get`).
    /// 3. Install Node dependencies (`npm install --omit=dev`).
    /// 4. Install Python dependencies (`pip install -r requirements.txt`).
    /// 5. Install Claude Code globally (`npm install -g @anthropic-ai/claude-code`).
    /// 6. Write a headless Lobster config and start the MCP server.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered.  All SSH commands are checked for a
    /// zero exit status; non-zero exits are turned into descriptive `Err`s.
    pub async fn provision_lobster(config: ProvisionConfig) -> Result<()> {
        let port_str = config.ssh_port.to_string();

        // ------------------------------------------------------------------
        // Step 1: Copy Lobster source into the VM
        // ------------------------------------------------------------------
        copy_source(&config, &port_str).await?;

        // ------------------------------------------------------------------
        // Step 2: Install Node.js + Python
        // ------------------------------------------------------------------
        install_runtime_deps(&port_str).await?;

        // ------------------------------------------------------------------
        // Step 3: Install Node.js dependencies (npm install)
        // ------------------------------------------------------------------
        run_remote(
            &port_str,
            &format!("cd {REMOTE_LOBSTER_DIR} && npm install --omit=dev 2>&1"),
            "npm install",
        )
        .await?;

        // ------------------------------------------------------------------
        // Step 4: Install Python dependencies (pip install -r requirements.txt)
        // ------------------------------------------------------------------
        run_remote(
            &port_str,
            &format!(
                "cd {REMOTE_LOBSTER_DIR} && \
                 [ -f requirements.txt ] && pip install --quiet -r requirements.txt || true"
            ),
            "pip install -r requirements.txt",
        )
        .await?;

        // ------------------------------------------------------------------
        // Step 5: Install Claude Code
        // ------------------------------------------------------------------
        run_remote(
            &port_str,
            "npm install -g @anthropic-ai/claude-code 2>&1",
            "npm install -g @anthropic-ai/claude-code",
        )
        .await?;

        // ------------------------------------------------------------------
        // Step 6: Write headless config and start Lobster MCP server
        // ------------------------------------------------------------------
        configure_and_start(&port_str).await?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Copy the host Lobster source tree into the VM at `/opt/lobster/`.
    ///
    /// Uses `scp -r` with the shared SSH options.  The remote directory is
    /// created first so the copy is idempotent (scp behaviour differs between
    /// OpenSSH versions when the destination already exists).
    async fn copy_source(config: &ProvisionConfig, port_str: &str) -> Result<()> {
        // Ensure destination directory exists.
        run_remote(
            port_str,
            &format!("mkdir -p {REMOTE_LOBSTER_DIR}"),
            "mkdir -p /opt/lobster",
        )
        .await?;

        let src = config
            .lobster_source_path
            .to_str()
            .context("lobster_source_path contains non-UTF-8 characters")?
            .to_owned();

        // scp -r <src>/. root@127.0.0.1:/opt/lobster/
        // The trailing "/." copies directory *contents*, avoiding an extra
        // nested directory when the destination already exists.
        let src_with_dot = format!("{src}/.");
        let dest = format!("{SSH_USER}@127.0.0.1:{REMOTE_LOBSTER_DIR}/");

        let mut args: Vec<String> = SSH_OPTS.iter().map(|s| s.to_string()).collect();
        args.extend([
            "-P".to_string(),
            port_str.to_string(),
            "-r".to_string(),
            src_with_dot,
            dest,
        ]);

        run_command("scp", &args, "scp -r lobster source into VM").await
    }

    /// Detect the package manager (apk or apt-get) and install Node.js +
    /// Python.  The `|| true` guards make the script idempotent.
    async fn install_runtime_deps(port_str: &str) -> Result<()> {
        // Alpine Linux (default for lightweight QEMU VMs).
        let alpine_cmd = "apk update --no-progress && \
                          apk add --no-progress nodejs npm python3 py3-pip 2>&1 || true";

        // Debian/Ubuntu fallback.
        let debian_cmd = "export DEBIAN_FRONTEND=noninteractive && \
                          apt-get update -qq && \
                          apt-get install -y -q nodejs npm python3 python3-pip 2>&1 || true";

        // Attempt Alpine first; fall back to apt-get if apk is absent.
        let combined = format!(
            "command -v apk > /dev/null 2>&1 && ({alpine_cmd}) || ({debian_cmd})"
        );

        run_remote(port_str, &combined, "install Node.js + Python").await
    }

    /// Write a minimal headless Lobster config and launch the MCP server.
    ///
    /// * `LOBSTER_HEADLESS=true` — disables any GUI / browser dependencies.
    /// * `LOBSTER_PORT=8080` — MCP server listens on a known internal port.
    /// * Uses `nohup … &` so the SSH session can exit without killing the server.
    async fn configure_and_start(port_str: &str) -> Result<()> {
        let script = format!(
            r#"
set -e

# Write headless environment config (idempotent).
mkdir -p {REMOTE_LOBSTER_DIR}/config
cat > {REMOTE_LOBSTER_DIR}/config/headless.env << 'ENVEOF'
LOBSTER_HEADLESS=true
LOBSTER_PORT=8080
NODE_ENV=production
ENVEOF

# Kill any existing lobster process (idempotent restart).
pkill -f "node.*lobster" 2>/dev/null || true

export LOBSTER_HEADLESS=true
export LOBSTER_PORT=8080
export NODE_ENV=production

nohup node {REMOTE_LOBSTER_DIR}/src/mcp/inbox_server.js \
    --headless \
    >> /var/log/lobster.log 2>&1 &

echo "Lobster MCP server started (PID $!)"
"#
        );

        run_remote(port_str, &script, "configure and start Lobster MCP server").await
    }

    /// Run a shell command inside the VM via `ssh`.
    ///
    /// Returns `Ok(())` on exit status 0, `Err` otherwise.
    async fn run_remote(port_str: &str, cmd: &str, label: &str) -> Result<()> {
        let target = format!("{SSH_USER}@127.0.0.1");

        let mut args: Vec<String> = SSH_OPTS.iter().map(|s| s.to_string()).collect();
        args.extend([
            "-p".to_string(),
            port_str.to_string(),
            target,
            cmd.to_string(),
        ]);

        run_command("ssh", &args, label).await
    }

    /// Spawn an external command, wait for it, and map non-zero exit to `Err`.
    async fn run_command(program: &str, args: &[String], label: &str) -> Result<()> {
        let status = Command::new(program)
            .args(args)
            .status()
            .await
            .with_context(|| format!("failed to spawn `{program}` for: {label}"))?;

        if status.success() {
            Ok(())
        } else {
            bail!(
                "`{program}` failed (exit {}) during: {label}",
                status.code().unwrap_or(-1)
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Linux stub — keeps the crate compilable on CI / Linux dev machines
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::ProvisionConfig;
    use anyhow::{bail, Result};

    /// Linux stub: VM provisioning is not supported outside macOS.
    pub async fn wait_for_vm_ready(_ssh_port: u16, _timeout_secs: u64) -> Result<()> {
        bail!("VM provisioning is only supported on macOS (QEMU/HVF target)")
    }

    /// Linux stub: VM provisioning is not supported outside macOS.
    pub async fn provision_lobster(_config: ProvisionConfig) -> Result<()> {
        bail!("VM provisioning is only supported on macOS (QEMU/HVF target)")
    }
}

// ---------------------------------------------------------------------------
// Re-export the platform-appropriate implementations
// ---------------------------------------------------------------------------

pub use imp::{provision_lobster, wait_for_vm_ready};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Helper: construct a `ProvisionConfig` with dummy paths.
    fn dummy_config() -> ProvisionConfig {
        ProvisionConfig {
            lobster_source_path: PathBuf::from("/tmp/lobster-src"),
            vm_serial_socket: PathBuf::from("/tmp/vm.sock"),
            ssh_port: 2222,
        }
    }

    #[test]
    fn provision_config_fields_are_accessible() {
        let cfg = dummy_config();
        assert_eq!(cfg.ssh_port, 2222);
        assert_eq!(cfg.lobster_source_path, PathBuf::from("/tmp/lobster-src"));
        assert_eq!(cfg.vm_serial_socket, PathBuf::from("/tmp/vm.sock"));
    }

    #[test]
    fn provision_config_is_cloneable() {
        let cfg = dummy_config();
        let cloned = cfg.clone();
        assert_eq!(cloned.ssh_port, cfg.ssh_port);
        assert_eq!(cloned.lobster_source_path, cfg.lobster_source_path);
    }

    /// On non-macOS platforms the stubs must return `Err` immediately.
    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn stub_wait_for_vm_ready_returns_err() {
        let result = wait_for_vm_ready(2222, 1).await;
        assert!(result.is_err(), "stub must return Err on non-macOS");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("macOS"),
            "error message should mention macOS, got: {msg}"
        );
    }

    /// On non-macOS platforms provision_lobster must return `Err` immediately.
    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn stub_provision_lobster_returns_err() {
        let result = provision_lobster(dummy_config()).await;
        assert!(result.is_err(), "stub must return Err on non-macOS");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("macOS"),
            "error message should mention macOS, got: {msg}"
        );
    }

    /// Verify `ProvisionConfig` implements `Debug` (useful for logging).
    #[test]
    fn provision_config_implements_debug() {
        let cfg = dummy_config();
        let debug_str = format!("{cfg:?}");
        assert!(debug_str.contains("ProvisionConfig"));
        assert!(debug_str.contains("2222"));
    }
}
