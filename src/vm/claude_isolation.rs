//! Claude Code isolation inside the VM.
//!
//! This module ensures that `claude --dangerously-skip-permissions` runs
//! inside the VM and can only see the VM's filesystem, never the host.
//!
//! ## Environment Variables Required Inside the VM
//!
//! Claude Code needs the following env vars set before launch:
//!
//! * `ANTHROPIC_API_KEY` — Anthropic API key for Claude Code.
//! * `HOME` — Set to `/opt/lobster` so `.claude/` config is self-contained
//!   inside the VM and never references host credentials.
//! * `CLAUDE_HOME` — Explicit override for the `.claude/` config directory:
//!   `/opt/lobster/.claude`.
//! * `PATH` — Must include the npm global bin directory so that the `claude`
//!   binary installed by `npm install -g @anthropic-ai/claude-code` is
//!   reachable: `/usr/local/bin:/usr/bin:/bin`.
//!
//! MCP server configuration lives in `/opt/lobster/.claude/claude_desktop_config.json`
//! inside the VM.  Only servers whose names appear in
//! [`ClaudeIsolationConfig::allowed_mcp_servers`] should be present in that
//! file.  Any other MCP server registered in the host's Claude config is
//! irrelevant and will not be visible to the guest instance.
//!
//! ## Isolation Guarantees
//!
//! The VM is started by vfkit (Virtualization.framework).  By default it has:
//! * No host filesystem mounts except the explicit virtiofs share at
//!   `/lobster-drop` (the drop folder).
//! * No direct internet access — all network leaves through a managed
//!   vsock/TCP channel provided by the bisque app.
//!
//! [`verify_claude_isolation`] confirms these guarantees at runtime by running
//! a small verification suite over SSH and returning a structured
//! [`IsolationReport`].
//!
//! ## Session Management
//!
//! [`ClaudeSessionManager`] tracks active Claude Code invocations in the VM.
//! Each session runs in its own tmux window so it can be attached to for
//! debugging and is cleanly namespaced from other sessions.
//!
//! # Platform Gating
//!
//! The full implementation is compiled only on macOS (`#[cfg(target_os = "macos")]`).
//! On Linux a minimal stub is provided so the crate still compiles on CI and
//! Linux development machines.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public types (available on all platforms)
// ---------------------------------------------------------------------------

/// Configuration for launching Claude Code inside the VM.
///
/// Constructed by the caller and passed to [`launch_claude_in_vm`] or used to
/// initialise a [`ClaudeSessionManager`].  All fields are plain data — there
/// is no hidden mutable state.
#[derive(Debug, Clone)]
pub struct ClaudeIsolationConfig {
    /// Host-side port that is forwarded to the VM's SSH daemon (port 22).
    ///
    /// This is the same port used by [`super::provisioning::ProvisionConfig`].
    /// Typical value: `2222`.
    pub vm_ssh_port: u16,

    /// Absolute path *inside the VM* that Claude Code will use as its working
    /// directory.  Should be within `/opt/lobster/` or `/lobster-drop/`.
    ///
    /// Example: `"/opt/lobster"`.
    pub vm_working_dir: String,

    /// Environment variables injected into the Claude Code process inside the
    /// VM.
    ///
    /// At a minimum this must contain `ANTHROPIC_API_KEY`.  Other useful keys:
    /// `HOME`, `CLAUDE_HOME`, `PATH`.  Values are shell-quoted when passed
    /// over SSH.
    pub claude_env_vars: HashMap<String, String>,

    /// Names of MCP servers that Claude Code is allowed to connect to inside
    /// the VM.
    ///
    /// These correspond to keys in
    /// `/opt/lobster/.claude/claude_desktop_config.json` inside the VM.  The
    /// list is informational — it is used by documentation and future
    /// enforcement logic but is not currently validated at runtime.
    pub allowed_mcp_servers: Vec<String>,
}

/// A single entry in an [`IsolationReport`].
///
/// The tuple fields are:
/// 0. Human-readable check name.
/// 1. `true` if the check passed (isolation held).
/// 2. Diagnostic message with extra detail.
pub type IsolationCheckResult = (String, bool, String);

/// Summary of a full isolation verification run produced by
/// [`verify_claude_isolation`].
#[derive(Debug, Clone)]
pub struct IsolationReport {
    /// `true` only when every individual check passed.
    pub all_checks_passed: bool,

    /// Individual check results in the order they were run.
    pub details: Vec<IsolationCheckResult>,
}

// ---------------------------------------------------------------------------
// macOS implementation
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod imp {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use anyhow::{Context, Result, bail};
    use tokio::process::Command;
    use tracing::{debug, info, warn};

    use super::{ClaudeIsolationConfig, IsolationCheckResult, IsolationReport};

    // -----------------------------------------------------------------------
    // Constants
    // -----------------------------------------------------------------------

    /// Remote user inside the VM.
    const SSH_USER: &str = "root";

    /// SSH options shared by every ssh invocation.
    ///
    /// * `StrictHostKeyChecking=no` — VM images are ephemeral.
    /// * `UserKnownHostsFile=/dev/null` — avoid polluting the host known_hosts.
    /// * `LogLevel=ERROR` — suppress banner noise.
    /// * `BatchMode=yes` — fail immediately if a password prompt would appear.
    /// * `ConnectTimeout=5` — don't hang waiting for sshd.
    const SSH_OPTS: &[&str] = &[
        "-o", "StrictHostKeyChecking=no",
        "-o", "UserKnownHostsFile=/dev/null",
        "-o", "LogLevel=ERROR",
        "-o", "BatchMode=yes",
        "-o", "ConnectTimeout=5",
    ];

    // -----------------------------------------------------------------------
    // ClaudeSessionManager
    // -----------------------------------------------------------------------

    /// A live Claude Code session running inside the VM.
    #[derive(Debug, Clone)]
    pub struct ClaudeSession {
        /// Unique identifier for this session (used as the tmux window name).
        pub id: String,
        /// The prompt that was passed to Claude Code when the session started.
        pub prompt_summary: String,
        /// Wall-clock time the session was started (Unix timestamp seconds).
        pub started_at: u64,
    }

    /// Tracks active Claude Code sessions inside the VM.
    ///
    /// Each session is launched in a dedicated tmux window so sessions are
    /// independent and can be inspected without disturbing each other.
    ///
    /// The manager holds a shared, mutex-protected list of known sessions.
    /// Starting/stopping sessions is done via SSH — the manager does **not**
    /// hold open SSH connections.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use bisque_computer::vm::claude_isolation::{ClaudeIsolationConfig, ClaudeSessionManager};
    /// use std::collections::HashMap;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = ClaudeIsolationConfig {
    ///     vm_ssh_port: 2222,
    ///     vm_working_dir: "/opt/lobster".into(),
    ///     claude_env_vars: HashMap::new(),
    ///     allowed_mcp_servers: vec![],
    /// };
    /// let manager = ClaudeSessionManager::new();
    /// let session_id = manager.start_session(&config, "summarise /opt/lobster/README.md").await?;
    /// let sessions = manager.list_sessions();
    /// manager.stop_session(&config, &session_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[derive(Debug, Clone)]
    pub struct ClaudeSessionManager {
        sessions: Arc<Mutex<Vec<ClaudeSession>>>,
    }

    impl ClaudeSessionManager {
        /// Create a new, empty session manager.
        pub fn new() -> Self {
            Self {
                sessions: Arc::new(Mutex::new(Vec::new())),
            }
        }

        /// Launch Claude Code inside the VM in a new tmux window.
        ///
        /// The session id is derived from the current Unix timestamp and a
        /// short hash of the prompt, producing a stable and unique name like
        /// `claude-1700000000-a3f2`.
        ///
        /// Returns the session id so the caller can later call
        /// [`stop_session`].
        pub async fn start_session(
            &self,
            config: &ClaudeIsolationConfig,
            prompt: &str,
        ) -> Result<String> {
            let session_id = generate_session_id(prompt);
            let summary: String = prompt.chars().take(60).collect();

            // Build the shell snippet that creates a tmux window and runs
            // Claude Code inside it.
            let env_exports = build_env_exports(&config.claude_env_vars);
            let safe_prompt = shell_escape(prompt);
            let working_dir = &config.vm_working_dir;

            let script = format!(
                r#"
tmux new-window -t lobster -n {session_id} \; \
  send-keys -t lobster:{session_id} \
    '{env_exports} cd {working_dir} && claude --dangerously-skip-permissions --no-update-notifier -p {safe_prompt} 2>/var/log/claude-{session_id}.log' Enter
"#
            );

            run_remote(config.vm_ssh_port, &script, &format!("start session {session_id}"))
                .await
                .with_context(|| format!("failed to start Claude session {session_id}"))?;

            let started_at = unix_now();
            let session = ClaudeSession {
                id: session_id.clone(),
                prompt_summary: summary,
                started_at,
            };

            self.sessions
                .lock()
                .expect("sessions mutex poisoned")
                .push(session);

            info!(session_id = %session_id, "Claude session started in VM");
            Ok(session_id)
        }

        /// Kill an active Claude Code session by closing its tmux window.
        ///
        /// The session is removed from the internal list regardless of whether
        /// the SSH command succeeds (the window may have already exited).
        pub async fn stop_session(
            &self,
            config: &ClaudeIsolationConfig,
            session_id: &str,
        ) -> Result<()> {
            let script = format!(
                "tmux kill-window -t lobster:{session_id} 2>/dev/null || true"
            );

            // Best-effort: log a warning but do not propagate SSH errors, since
            // the session may have already exited on its own.
            if let Err(e) =
                run_remote(config.vm_ssh_port, &script, &format!("stop session {session_id}"))
                    .await
            {
                warn!(session_id = %session_id, error = %e, "stop_session SSH command failed (ignoring)");
            }

            self.sessions
                .lock()
                .expect("sessions mutex poisoned")
                .retain(|s| s.id != session_id);

            info!(session_id = %session_id, "Claude session stopped");
            Ok(())
        }

        /// Return a snapshot of the currently tracked sessions.
        ///
        /// Note: this reflects the manager's in-memory state.  A session that
        /// exited on its own (e.g., Claude Code finished) will still appear
        /// here until [`stop_session`] is called.
        pub fn list_sessions(&self) -> Vec<ClaudeSession> {
            self.sessions
                .lock()
                .expect("sessions mutex poisoned")
                .clone()
        }
    }

    impl Default for ClaudeSessionManager {
        fn default() -> Self {
            Self::new()
        }
    }

    // -----------------------------------------------------------------------
    // launch_claude_in_vm
    // -----------------------------------------------------------------------

    /// SSH into the VM and run `claude --dangerously-skip-permissions -p "<prompt>"`.
    ///
    /// Captures and returns the combined stdout of the Claude Code process.
    /// stderr is redirected to `/var/log/claude-launch.log` inside the VM to
    /// keep the captured output clean.
    ///
    /// # Environment
    ///
    /// The env vars in [`ClaudeIsolationConfig::claude_env_vars`] are exported
    /// before the `claude` invocation.  At a minimum `ANTHROPIC_API_KEY` must
    /// be present or Claude Code will refuse to start.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the SSH connection fails, the command exits with a
    /// non-zero status, or the output cannot be decoded as UTF-8.
    pub async fn launch_claude_in_vm(
        config: &ClaudeIsolationConfig,
        prompt: &str,
    ) -> Result<String> {
        let env_exports = build_env_exports(&config.claude_env_vars);
        let safe_prompt = shell_escape(prompt);
        let working_dir = &config.vm_working_dir;

        let script = format!(
            "{env_exports} cd {working_dir} && \
             claude --dangerously-skip-permissions --no-update-notifier \
               -p {safe_prompt} \
               2>/var/log/claude-launch.log"
        );

        let port_str = config.vm_ssh_port.to_string();
        let target = format!("{SSH_USER}@127.0.0.1");

        let mut args: Vec<String> = SSH_OPTS.iter().map(|s| s.to_string()).collect();
        args.extend([
            "-p".to_string(),
            port_str,
            target,
            script,
        ]);

        debug!(
            port = config.vm_ssh_port,
            prompt_len = prompt.len(),
            "Launching Claude Code in VM"
        );

        let output = Command::new("ssh")
            .args(&args)
            .output()
            .await
            .context("failed to spawn ssh for launch_claude_in_vm")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!(
                "claude --dangerously-skip-permissions exited {} in VM: {}",
                output.status.code().unwrap_or(-1),
                stderr.trim()
            );
        }

        let stdout = String::from_utf8(output.stdout)
            .context("Claude Code output is not valid UTF-8")?;

        info!(
            port = config.vm_ssh_port,
            output_bytes = stdout.len(),
            "Claude Code completed in VM"
        );

        Ok(stdout)
    }

    // -----------------------------------------------------------------------
    // verify_claude_isolation
    // -----------------------------------------------------------------------

    /// Run the full isolation verification suite and return an [`IsolationReport`].
    ///
    /// ## Checks performed
    ///
    /// 1. **Host `/private/etc/passwd` inaccessible** — probes for
    ///    `/private/etc/passwd`, a macOS-only symlink target that does not
    ///    exist inside a Linux VM.  The check passes if the path is absent.
    ///
    /// 2. **Host sentinel file inaccessible** — writes a sentinel file to a
    ///    host-only path (`/tmp/.bisque-host-sentinel`) that is NOT included in
    ///    the virtiofs share.  The check passes if the VM cannot see that file.
    ///    (Because the sentinel is written on the host, not in the VM, the VM
    ///    should never find it.)
    ///
    /// 3. **No direct internet access** — attempts `curl -sf --max-time 3
    ///    https://example.com` inside the VM.  The check passes if curl fails
    ///    (exit non-zero), confirming the VM has no direct outbound internet
    ///    route.
    ///
    /// 4. **`/lobster-drop` readable** — confirms that the virtiofs share is
    ///    mounted and the VM can list its contents.  The check passes if `ls
    ///    /lobster-drop` succeeds.
    pub async fn verify_claude_isolation(ssh_port: u16) -> Result<IsolationReport> {
        let mut details: Vec<IsolationCheckResult> = Vec::new();

        // ------------------------------------------------------------------
        // Check 1: macOS-only host path must not exist inside the VM.
        // ------------------------------------------------------------------
        {
            let (passed, msg) = check_path_absent(ssh_port, "/private/etc/passwd").await;
            details.push((
                "host /private/etc/passwd inaccessible".to_string(),
                passed,
                msg,
            ));
        }

        // ------------------------------------------------------------------
        // Check 2: Host sentinel file must not appear in the VM.
        // ------------------------------------------------------------------
        {
            let sentinel_path = "/tmp/.bisque-host-sentinel";
            // Write the sentinel file on the *host* (not in the VM).
            // The VM should not be able to see it because it has no access to
            // the host /tmp.
            let _ = tokio::fs::write(sentinel_path, b"bisque-host-only\n").await;

            let (passed, msg) = check_path_absent(ssh_port, sentinel_path).await;
            details.push((
                "host sentinel file inaccessible".to_string(),
                passed,
                msg,
            ));

            // Clean up the sentinel file on the host.
            let _ = tokio::fs::remove_file(sentinel_path).await;
        }

        // ------------------------------------------------------------------
        // Check 3: No direct internet access from inside the VM.
        // ------------------------------------------------------------------
        {
            let (passed, msg) = check_no_internet(ssh_port).await;
            details.push((
                "no direct internet access from VM".to_string(),
                passed,
                msg,
            ));
        }

        // ------------------------------------------------------------------
        // Check 4: /lobster-drop virtiofs share is readable.
        // ------------------------------------------------------------------
        {
            let (passed, msg) = check_lobster_drop_readable(ssh_port).await;
            details.push((
                "/lobster-drop virtiofs share readable".to_string(),
                passed,
                msg,
            ));
        }

        let all_checks_passed = details.iter().all(|(_, passed, _)| *passed);

        if all_checks_passed {
            info!(port = ssh_port, "All isolation checks passed");
        } else {
            let failed: Vec<&str> = details
                .iter()
                .filter(|(_, passed, _)| !passed)
                .map(|(name, _, _)| name.as_str())
                .collect();
            warn!(port = ssh_port, ?failed, "Isolation check(s) FAILED");
        }

        Ok(IsolationReport {
            all_checks_passed,
            details,
        })
    }

    // -----------------------------------------------------------------------
    // Isolation check helpers
    // -----------------------------------------------------------------------

    /// Returns `(true, msg)` when `path` does **not** exist inside the VM
    /// (i.e., isolation holds).
    async fn check_path_absent(ssh_port: u16, path: &str) -> (bool, String) {
        // `test -e <path>` exits 0 if the file exists, 1 if it does not.
        // We want it to NOT exist — so a non-zero exit from `test` is a pass.
        let script = format!("test -e {path} && echo PRESENT || echo ABSENT");

        match run_remote_output(ssh_port, &script).await {
            Ok(output) => {
                let trimmed = output.trim();
                if trimmed == "ABSENT" {
                    (true, format!("{path} correctly absent from VM"))
                } else {
                    (
                        false,
                        format!("ISOLATION FAILURE: {path} is accessible inside the VM"),
                    )
                }
            }
            Err(e) => (
                false,
                format!("SSH error while checking {path}: {e}"),
            ),
        }
    }

    /// Returns `(true, msg)` when the VM cannot reach the internet directly.
    async fn check_no_internet(ssh_port: u16) -> (bool, String) {
        // curl exits non-zero when the connection fails.  We set a short
        // timeout so the check completes quickly even when the VM has no
        // firewall block and the connection would otherwise hang.
        let script = "curl -sf --max-time 3 https://example.com > /dev/null 2>&1 \
                      && echo REACHABLE || echo UNREACHABLE";

        match run_remote_output(ssh_port, script).await {
            Ok(output) => {
                let trimmed = output.trim();
                if trimmed == "UNREACHABLE" {
                    (true, "internet unreachable from VM (isolation OK)".to_string())
                } else {
                    (
                        false,
                        "ISOLATION FAILURE: VM can reach the internet directly".to_string(),
                    )
                }
            }
            Err(e) => (
                false,
                format!("SSH error during internet reachability check: {e}"),
            ),
        }
    }

    /// Returns `(true, msg)` when `/lobster-drop` is mounted and readable.
    async fn check_lobster_drop_readable(ssh_port: u16) -> (bool, String) {
        let script = "ls /lobster-drop > /dev/null 2>&1 && echo READABLE || echo NOT_READABLE";

        match run_remote_output(ssh_port, script).await {
            Ok(output) => {
                let trimmed = output.trim();
                if trimmed == "READABLE" {
                    (true, "/lobster-drop is mounted and readable".to_string())
                } else {
                    (
                        false,
                        "/lobster-drop is NOT readable — virtiofs share may not be mounted"
                            .to_string(),
                    )
                }
            }
            Err(e) => (
                false,
                format!("SSH error while checking /lobster-drop: {e}"),
            ),
        }
    }

    // -----------------------------------------------------------------------
    // SSH helpers
    // -----------------------------------------------------------------------

    /// Run a shell command inside the VM and capture its stdout as a `String`.
    async fn run_remote_output(ssh_port: u16, cmd: &str) -> Result<String> {
        let port_str = ssh_port.to_string();
        let target = format!("{SSH_USER}@127.0.0.1");

        let mut args: Vec<String> = SSH_OPTS.iter().map(|s| s.to_string()).collect();
        args.extend(["-p".to_string(), port_str, target, cmd.to_string()]);

        let output = Command::new("ssh")
            .args(&args)
            .output()
            .await
            .with_context(|| format!("failed to spawn ssh for remote command: {cmd}"))?;

        // We do not treat non-zero exit as an error here because many of the
        // isolation checks rely on the command returning a specific exit code.
        // The caller interprets the captured stdout to decide pass/fail.
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Run a shell command inside the VM, returning `Err` on non-zero exit.
    async fn run_remote(ssh_port: u16, cmd: &str, label: &str) -> Result<()> {
        let port_str = ssh_port.to_string();
        let target = format!("{SSH_USER}@127.0.0.1");

        let mut args: Vec<String> = SSH_OPTS.iter().map(|s| s.to_string()).collect();
        args.extend(["-p".to_string(), port_str, target, cmd.to_string()]);

        let status = Command::new("ssh")
            .args(&args)
            .status()
            .await
            .with_context(|| format!("failed to spawn ssh for: {label}"))?;

        if status.success() {
            Ok(())
        } else {
            bail!(
                "ssh command failed (exit {}) during: {label}",
                status.code().unwrap_or(-1)
            )
        }
    }

    // -----------------------------------------------------------------------
    // Pure helper functions
    // -----------------------------------------------------------------------

    /// Build a sequence of `export KEY=VALUE` statements from a map.
    ///
    /// Values are single-quote-escaped so they survive the shell invocation
    /// safely.  The resulting string ends with a space so it can be prepended
    /// directly to another shell command.
    fn build_env_exports(vars: &HashMap<String, String>) -> String {
        let mut exports: Vec<String> = vars
            .iter()
            .map(|(k, v)| format!("export {}={}", k, shell_escape(v)))
            .collect();
        // Sort for deterministic output (useful in tests and logs).
        exports.sort();
        if exports.is_empty() {
            String::new()
        } else {
            format!("{} ", exports.join(" "))
        }
    }

    /// Wrap a string in single quotes, escaping any embedded single quotes.
    ///
    /// Single-quote escaping in POSIX sh: end the current quote, insert a
    /// literal `'` via `'\''`, then re-open the quote.
    fn shell_escape(s: &str) -> String {
        format!("'{}'", s.replace('\'', r"'\''"))
    }

    /// Return the current Unix timestamp in seconds.
    fn unix_now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs()
    }

    /// Derive a deterministic session id from the current timestamp and a
    /// short hash of the prompt text.
    fn generate_session_id(prompt: &str) -> String {
        let ts = unix_now();
        // Fold the prompt characters into a simple 16-bit checksum.
        let hash: u16 = prompt
            .bytes()
            .fold(0u16, |acc, b| acc.wrapping_add(b as u16));
        format!("claude-{ts}-{hash:04x}")
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn shell_escape_simple_string() {
            assert_eq!(shell_escape("hello world"), "'hello world'");
        }

        #[test]
        fn shell_escape_with_single_quote() {
            assert_eq!(shell_escape("it's"), "'it'\\''s'");
        }

        #[test]
        fn build_env_exports_empty() {
            let exports = build_env_exports(&HashMap::new());
            assert_eq!(exports, "");
        }

        #[test]
        fn build_env_exports_single_var() {
            let mut vars = HashMap::new();
            vars.insert("FOO".to_string(), "bar".to_string());
            let exports = build_env_exports(&vars);
            assert_eq!(exports, "export FOO='bar' ");
        }

        #[test]
        fn build_env_exports_deterministic_order() {
            let mut vars = HashMap::new();
            vars.insert("Z_VAR".to_string(), "z".to_string());
            vars.insert("A_VAR".to_string(), "a".to_string());
            let exports = build_env_exports(&vars);
            // Keys are sorted: A_VAR comes before Z_VAR.
            let a_pos = exports.find("A_VAR").unwrap();
            let z_pos = exports.find("Z_VAR").unwrap();
            assert!(a_pos < z_pos);
        }

        #[test]
        fn generate_session_id_format() {
            let id = generate_session_id("test prompt");
            assert!(id.starts_with("claude-"), "id = {id}");
            // claude-<timestamp>-<hex>
            let parts: Vec<&str> = id.splitn(3, '-').collect();
            assert_eq!(parts.len(), 3);
            assert_eq!(parts[0], "claude");
        }

        #[test]
        fn isolation_report_all_passed() {
            let details = vec![
                ("check a".to_string(), true, "ok".to_string()),
                ("check b".to_string(), true, "ok".to_string()),
            ];
            let all = details.iter().all(|(_, p, _)| *p);
            assert!(all);
        }

        #[test]
        fn isolation_report_one_failed() {
            let details = vec![
                ("check a".to_string(), true, "ok".to_string()),
                ("check b".to_string(), false, "fail".to_string()),
            ];
            let all = details.iter().all(|(_, p, _)| *p);
            assert!(!all);
        }
    }
}

// ---------------------------------------------------------------------------
// Linux / non-macOS stub
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "macos"))]
mod imp {
    use anyhow::{bail, Result};

    use super::{ClaudeIsolationConfig, IsolationReport};

    /// Placeholder session type for non-macOS builds.
    #[derive(Debug, Clone)]
    pub struct ClaudeSession {
        pub id: String,
        pub prompt_summary: String,
        pub started_at: u64,
    }

    /// Placeholder session manager — always errors on non-macOS platforms.
    #[derive(Debug, Clone, Default)]
    pub struct ClaudeSessionManager;

    impl ClaudeSessionManager {
        pub fn new() -> Self {
            Self
        }

        pub async fn start_session(
            &self,
            _config: &ClaudeIsolationConfig,
            _prompt: &str,
        ) -> Result<String> {
            bail!("Claude isolation requires macOS (Virtualization.framework via vfkit)")
        }

        pub async fn stop_session(
            &self,
            _config: &ClaudeIsolationConfig,
            _session_id: &str,
        ) -> Result<()> {
            bail!("Claude isolation requires macOS (Virtualization.framework via vfkit)")
        }

        pub fn list_sessions(&self) -> Vec<ClaudeSession> {
            Vec::new()
        }
    }

    /// Linux stub: always returns `Err`.
    pub async fn launch_claude_in_vm(
        _config: &ClaudeIsolationConfig,
        _prompt: &str,
    ) -> Result<String> {
        bail!("Claude isolation requires macOS (Virtualization.framework via vfkit)")
    }

    /// Linux stub: always returns `Err`.
    pub async fn verify_claude_isolation(_ssh_port: u16) -> Result<IsolationReport> {
        bail!("Claude isolation requires macOS (Virtualization.framework via vfkit)")
    }
}

// ---------------------------------------------------------------------------
// Re-export the platform-appropriate symbols
// ---------------------------------------------------------------------------

pub use imp::{ClaudeSession, ClaudeSessionManager, launch_claude_in_vm, verify_claude_isolation};
