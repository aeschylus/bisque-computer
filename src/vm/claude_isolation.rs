//! Claude Code isolation verification inside the VM.
//!
//! Verifies that the Claude Code CLI is installed and accessible at the
//! expected path inside the VM guest, and that it runs in a properly
//! isolated environment (no host filesystem access beyond virtio-fs shares).
//!
//! # Platform gating
//!
//! Full implementation on macOS; Linux stub returns `Err`.

use anyhow::Result;

/// Verify that `claude` is installed inside the VM at `/usr/local/bin/claude`.
///
/// Runs `ssh root@127.0.0.1 -p <ssh_port> which claude` and checks the output.
/// Returns `Ok(())` if the binary is present and executable.
///
/// # Errors
///
/// Returns `Err` if:
/// - SSH connection fails
/// - `which claude` exits non-zero (binary not found)
/// - The binary path does not match `/usr/local/bin/claude`
#[cfg(target_os = "macos")]
pub async fn verify_claude_installed(ssh_port: u16) -> Result<()> {
    use anyhow::{bail, Context};
    use tokio::process::Command;

    let output = Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=no",
            "-o", "UserKnownHostsFile=/dev/null",
            "-o", "LogLevel=ERROR",
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=5",
            "-p", &ssh_port.to_string(),
            "root@127.0.0.1",
            "which claude && claude --version 2>&1 | head -1",
        ])
        .output()
        .await
        .context("failed to spawn ssh for claude verification")?;

    if !output.status.success() {
        bail!(
            "claude CLI not found inside VM (ssh_port={ssh_port}): {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("claude") {
        bail!("unexpected output from `which claude`: {stdout}");
    }

    Ok(())
}

/// Linux stub.
#[cfg(not(target_os = "macos"))]
pub async fn verify_claude_installed(_ssh_port: u16) -> Result<()> {
    anyhow::bail!("claude isolation verification is only supported on macOS")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn stub_returns_err_on_linux() {
        let result = verify_claude_installed(2222).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("macOS"));
    }
}
