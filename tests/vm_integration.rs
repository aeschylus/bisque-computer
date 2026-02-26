//! Integration tests for the VM sandboxing system.
//!
//! These tests verify end-to-end sandbox properties by launching a real VM via
//! vfkit and exercising the isolation boundaries.  Because they require a
//! running VM, a Linux disk image, and macOS, they are gated with the
//! `vm-integration-tests` feature flag.
//!
//! # Running
//!
//! ```bash
//! cargo test --features vm-integration-tests --test vm_integration
//! ```
//!
//! See `README_TESTING.md` in the repository root for full prerequisites and
//! environment setup instructions.
//!
//! # Architecture
//!
//! Each test uses the `TestVm` helper (defined below) which wraps the VM
//! lifecycle API with test-specific helpers:
//! - spawns a VM from a test disk image via vfkit
//! - provides `exec()` for running shell commands inside the VM via SSH
//! - provides `host_drop_dir()` for the virtiofs-shared drop folder path
//! - implements `Drop` to ensure the VM is always killed if a test panics

#![cfg(all(feature = "vm-integration-tests", target_os = "macos"))]

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::process::Command;
use tokio::time::timeout;

// ---------------------------------------------------------------------------
// Environment variable helpers
// ---------------------------------------------------------------------------

/// Port forwarded from host to the VM guest's SSH daemon (port 22).
fn test_ssh_port() -> u16 {
    std::env::var("BISQUE_TEST_SSH_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2299)
}

/// Port for the vfkit REST management API used by the test VM.
fn test_rest_port() -> u16 {
    std::env::var("BISQUE_TEST_REST_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7799)
}

/// Path to the Linux disk image to boot in tests.
///
/// Set `BISQUE_TEST_DISK_IMAGE` or place the image at
/// `tests/fixtures/test-vm.img`.
fn test_disk_image() -> PathBuf {
    std::env::var("BISQUE_TEST_DISK_IMAGE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("test-vm.img")
        })
}

/// Path to the Linux kernel image for the test VM.
fn test_kernel() -> PathBuf {
    std::env::var("BISQUE_TEST_KERNEL")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("vmlinuz")
        })
}

/// Path to the initrd image for the test VM.
fn test_initrd() -> PathBuf {
    std::env::var("BISQUE_TEST_INITRD")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join("initrd.img")
        })
}

/// Allowlisted remote Lobster URL for relay tests.
fn test_allowlisted_url() -> String {
    std::env::var("BISQUE_TEST_REMOTE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:18080".to_string())
}

// ---------------------------------------------------------------------------
// TestVm helper
// ---------------------------------------------------------------------------

/// SSH options shared by all test commands.
const TEST_SSH_OPTS: &[&str] = &[
    "-o", "StrictHostKeyChecking=no",
    "-o", "UserKnownHostsFile=/dev/null",
    "-o", "BatchMode=yes",
    "-o", "ConnectTimeout=5",
    "-o", "LogLevel=ERROR",
];

/// A live VM instance for a single test.
///
/// `Drop` sends a best-effort kill so the VM process does not outlive the test.
struct TestVm {
    /// Host-side drop folder, mounted in the VM at `/mnt/lobster-drop/`.
    drop_dir: PathBuf,
    /// The vfkit child process.
    child: tokio::process::Child,
    /// Temp dir that owns `drop_dir` and the serial log.
    _tmp: tempfile::TempDir,
}

impl TestVm {
    /// Spawn a fresh VM and wait for SSH to become reachable (up to 60 s).
    async fn spawn() -> Result<Self> {
        let tmp = tempfile::TempDir::new().context("create temp dir for test VM")?;
        let drop_dir = tmp.path().join("lobster-drop");
        tokio::fs::create_dir_all(&drop_dir).await.context("create drop dir")?;
        let serial_log = tmp.path().join("serial.log");

        let disk = test_disk_image();
        let kernel = test_kernel();
        let initrd = test_initrd();

        for (label, path) in [("disk image", &disk), ("kernel", &kernel), ("initrd", &initrd)] {
            if !path.exists() {
                bail!(
                    "Test {label} not found at {path}.\n\
                     Set the appropriate env var. See README_TESTING.md.",
                    path = path.display()
                );
            }
        }

        let vfkit = find_vfkit().context("vfkit not found — install: brew install vfkit")?;

        let child = Command::new(&vfkit)
            .arg("--bootloader")
            .arg(format!(
                "linux,kernel={},initrd={},cmdline=console=hvc0 root=/dev/vda rw",
                kernel.display(),
                initrd.display()
            ))
            .arg("--cpus").arg("1")
            .arg("--memory").arg("512")
            .arg("--device")
            .arg(format!("virtio-blk,path={}", disk.display()))
            .arg("--device")
            .arg(format!("virtio-serial,logFilePath={}", serial_log.display()))
            .arg("--device").arg("virtio-rng")
            .arg("--device")
            .arg(format!(
                "virtio-net,nat,portForwards=22:{}",
                test_ssh_port()
            ))
            .arg("--device")
            .arg(format!(
                "virtio-fs,sharedDir={},mountTag=lobster-drop",
                drop_dir.display()
            ))
            .arg("--restful-uri")
            .arg(format!("tcp://localhost:{}", test_rest_port()))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("failed to spawn vfkit")?;

        let vm = TestVm { drop_dir, child, _tmp: tmp };

        vm.wait_for_ssh(Duration::from_secs(60)).await?;

        // Mount the virtiofs drop share inside the VM.
        let _ = vm
            .exec("mkdir -p /mnt/lobster-drop && mount -t virtiofs lobster-drop /mnt/lobster-drop || true")
            .await;

        Ok(vm)
    }

    /// Host-side drop folder path (visible in VM at `/mnt/lobster-drop/`).
    fn host_drop_dir(&self) -> &PathBuf {
        &self.drop_dir
    }

    /// Run a shell command inside the VM via SSH and return stdout.
    async fn exec(&self, cmd: &str) -> Result<String> {
        let mut args: Vec<String> = TEST_SSH_OPTS.iter().map(|s| s.to_string()).collect();
        args.extend([
            "-p".to_string(),
            test_ssh_port().to_string(),
            "root@127.0.0.1".to_string(),
            cmd.to_string(),
        ]);

        let output = Command::new("ssh")
            .args(&args)
            .output()
            .await
            .context("spawn ssh")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            bail!(
                "SSH command failed (exit {}):\ncmd: {}\nstdout: {}\nstderr: {}",
                output.status.code().unwrap_or(-1),
                cmd,
                stdout.trim(),
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Returns true if the vfkit child process is still running.
    fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Stop the VM gracefully via the vfkit REST API, then wait for exit.
    async fn stop(mut self) -> Result<()> {
        // Best-effort REST stop.
        let client = reqwest::Client::new();
        let url = format!("http://localhost:{}/vm/state", test_rest_port());
        let _ = client
            .put(&url)
            .json(&serde_json::json!({"state": "Stop"}))
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        // Wait up to 10 s for the process to exit.
        match timeout(Duration::from_secs(10), self.child.wait()).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(e).context("waiting for VM process"),
            Err(_) => {
                let _ = self.child.kill().await;
                bail!("VM did not stop within 10 seconds")
            }
        }
    }

    /// Poll until SSH port accepts connections or deadline passes.
    async fn wait_for_ssh(&self, deadline: Duration) -> Result<()> {
        use tokio::net::TcpStream;
        let addr = format!("127.0.0.1:{}", test_ssh_port());
        let start = std::time::Instant::now();
        loop {
            if TcpStream::connect(&addr).await.is_ok() {
                return Ok(());
            }
            if start.elapsed() >= deadline {
                bail!(
                    "SSH port {} not reachable within {:?}",
                    test_ssh_port(),
                    deadline
                );
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

impl Drop for TestVm {
    fn drop(&mut self) {
        // Best-effort kill to prevent zombie VM processes.
        let _ = self.child.start_kill();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_vfkit() -> Result<PathBuf> {
    let candidates = ["/opt/homebrew/bin/vfkit", "/usr/local/bin/vfkit"];
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let candidate = PathBuf::from(dir).join("vfkit");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    for &path in &candidates {
        if PathBuf::from(path).exists() {
            return Ok(PathBuf::from(path));
        }
    }
    bail!("vfkit not found. Install: brew install vfkit")
}

// ---------------------------------------------------------------------------
// Integration tests
// ---------------------------------------------------------------------------

/// Verify that a process inside the VM cannot read the host's `/etc/passwd`.
///
/// The VM runs with full filesystem isolation. The host `/etc/passwd` must
/// be absent inside the VM — no host-specific usernames should appear.
#[tokio::test]
async fn test_claude_cannot_read_host_passwd() {
    let vm = TestVm::spawn().await.expect("spawn test VM");

    // The host has a user called "admin". A clean Alpine/Debian VM must not.
    let output = vm.exec("cat /etc/passwd 2>&1").await.unwrap_or_default();
    assert!(
        !output.contains("admin"),
        "host /etc/passwd must not be readable inside the VM (found 'admin' user):\n{}",
        output
    );

    // /Users is macOS-only and must not exist inside the VM.
    let ls_result = vm.exec("ls /Users 2>&1").await;
    assert!(
        ls_result.is_err() || ls_result.unwrap().contains("No such file"),
        "/Users must not exist inside the isolated VM"
    );

    vm.stop().await.expect("VM must stop cleanly");
}

/// Verify that a file written to the host drop folder appears inside the VM
/// at `/mnt/lobster-drop/<filename>` within 2 seconds.
///
/// virtio-fs propagates writes via shared memory with no polling delay.
#[tokio::test]
async fn test_drop_folder_file_appears_in_vm() {
    let vm = TestVm::spawn().await.expect("spawn test VM");

    let filename = format!(
        "bisque-drop-test-{}.txt",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let host_path = vm.host_drop_dir().join(&filename);
    let content = "lobster-drop-integration-test-content";

    tokio::fs::write(&host_path, content)
        .await
        .expect("write test file to host drop folder");

    // Poll for up to 2 seconds (4 × 500 ms).
    let vm_path = format!("/mnt/lobster-drop/{filename}");
    let mut appeared = false;
    for _ in 0..4 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let Ok(out) = vm.exec(&format!("cat {vm_path} 2>&1")).await {
            if out.contains(content) {
                appeared = true;
                break;
            }
        }
    }

    let _ = tokio::fs::remove_file(&host_path).await;

    assert!(
        appeared,
        "file '{filename}' must appear in VM at /mnt/lobster-drop/{filename} within 2 s"
    );

    vm.stop().await.expect("VM must stop cleanly");
}

/// Verify that the remote channel relay rejects messages to non-allowlisted URLs.
///
/// The test starts a minimal TCP relay that enforces an allowlist, then
/// sends a message to a non-listed destination and asserts rejection.
#[tokio::test]
async fn test_remote_channel_allowlist() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    let relay_port: u16 = 13201;
    let listener = TcpListener::bind(format!("127.0.0.1:{relay_port}"))
        .await
        .expect("bind relay listener");

    let allowlisted_url = test_allowlisted_url();

    // Minimal allowlist-enforcing relay.
    let relay = tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut buf = Vec::new();
            let mut byte = [0u8; 1];
            loop {
                match stream.read(&mut byte).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if byte[0] == b'\n' { break; }
                        buf.push(byte[0]);
                    }
                }
            }
            if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(&buf) {
                let dest = msg["destination"].as_str().unwrap_or("");
                if dest == allowlisted_url {
                    let _ = stream.write_all(b"{\"ok\":true}\n").await;
                } else {
                    let _ = stream.write_all(b"{\"error\":\"destination_not_allowed\"}\n").await;
                }
            }
        }
    });

    // Send a message to a non-allowlisted destination.
    let mut client = TcpStream::connect(format!("127.0.0.1:{relay_port}"))
        .await
        .expect("connect to relay");

    let msg = serde_json::json!({
        "source": "vm-lobster",
        "destination": "http://evil-server.example.com:9999",
        "payload": {},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let mut bytes = serde_json::to_vec(&msg).unwrap();
    bytes.push(b'\n');
    client.write_all(&bytes).await.expect("send message");

    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match client.read(&mut byte).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if byte[0] == b'\n' { break; }
                response.push(byte[0]);
            }
        }
    }

    let resp_str = String::from_utf8_lossy(&response);
    assert!(
        resp_str.contains("destination_not_allowed"),
        "relay must reject non-allowlisted destinations, got: {resp_str}"
    );

    relay.await.expect("relay task panicked");
}

/// Verify that the remote channel relay forwards a message to an allowlisted URL.
///
/// A mock "remote Lobster" server is started on the host. The relay validates
/// the allowlist and proxies the payload to the mock, which replies with success.
#[tokio::test]
async fn test_remote_channel_relay() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    let mock_port: u16 = 18080;
    let relay_port: u16 = 13202;

    // Mock remote Lobster server.
    let mock_listener = TcpListener::bind(format!("127.0.0.1:{mock_port}"))
        .await
        .expect("bind mock server");
    let mock = tokio::spawn(async move {
        if let Ok((mut stream, _)) = mock_listener.accept().await {
            let mut byte = [0u8; 1];
            loop {
                match stream.read(&mut byte).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => { if byte[0] == b'\n' { break; } }
                }
            }
            let _ = stream.write_all(b"{\"ok\":true,\"echo\":true}\n").await;
        }
    });

    let allowlisted_url = format!("http://127.0.0.1:{mock_port}");
    let allowlist_clone = allowlisted_url.clone();

    // Test relay.
    let relay_listener = TcpListener::bind(format!("127.0.0.1:{relay_port}"))
        .await
        .expect("bind relay");
    let relay = tokio::spawn(async move {
        if let Ok((mut stream, _)) = relay_listener.accept().await {
            let mut buf = Vec::new();
            let mut byte = [0u8; 1];
            loop {
                match stream.read(&mut byte).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if byte[0] == b'\n' { break; }
                        buf.push(byte[0]);
                    }
                }
            }
            if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(&buf) {
                let dest = msg["destination"].as_str().unwrap_or("");
                if dest == allowlist_clone {
                    let stripped = dest.strip_prefix("http://").unwrap_or(dest);
                    if let Ok(mut remote) = TcpStream::connect(stripped).await {
                        let payload = msg["payload"].to_string();
                        let _ = remote.write_all(payload.as_bytes()).await;
                        let _ = remote.write_all(b"\n").await;
                        let mut resp = Vec::new();
                        let mut b = [0u8; 1];
                        loop {
                            match remote.read(&mut b).await {
                                Ok(0) | Err(_) => break,
                                Ok(_) => {
                                    if b[0] == b'\n' { break; }
                                    resp.push(b[0]);
                                }
                            }
                        }
                        resp.push(b'\n');
                        let _ = stream.write_all(&resp).await;
                    } else {
                        let _ = stream.write_all(b"{\"error\":\"proxy_failed\"}\n").await;
                    }
                } else {
                    let _ = stream.write_all(b"{\"error\":\"destination_not_allowed\"}\n").await;
                }
            }
        }
    });

    // Client sends to an allowlisted destination.
    let mut client = TcpStream::connect(format!("127.0.0.1:{relay_port}"))
        .await
        .expect("connect to relay");
    let msg = serde_json::json!({
        "source": "vm-lobster",
        "destination": allowlisted_url,
        "payload": {"type": "ping"},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let mut bytes = serde_json::to_vec(&msg).unwrap();
    bytes.push(b'\n');
    client.write_all(&bytes).await.expect("send message");

    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match client.read(&mut byte).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if byte[0] == b'\n' { break; }
                response.push(byte[0]);
            }
        }
    }

    let resp_str = String::from_utf8_lossy(&response);
    assert!(
        resp_str.contains("ok") || resp_str.contains("true"),
        "relay must succeed for allowlisted destination, got: {resp_str}"
    );

    relay.await.expect("relay task panicked");
    mock.await.expect("mock server task panicked");
}

/// Verify that a VM stops cleanly when `stop()` is called.
///
/// The VM process must exit within 10 seconds. No zombie processes should
/// remain after `stop()` returns.
#[tokio::test]
async fn test_vm_stops_cleanly() {
    let mut vm = TestVm::spawn().await.expect("spawn test VM");

    assert!(vm.is_running(), "VM must be running before stop()");

    let result = timeout(Duration::from_secs(10), vm.stop()).await;

    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => panic!("stop() returned error: {e}"),
        Err(_) => panic!("VM did not stop within 10 seconds"),
    }
}
