//! Remote Lobster communication channel via TCP relay.
//!
//! Provides a controlled channel for the sandboxed VM Lobster instance to
//! communicate with remote Lobster instances, with the bisque app acting as a
//! proxy/relay. The VM has no direct internet access — all outbound
//! communication must flow through this channel.
//!
//! ## Architecture
//!
//! ```text
//! VM Lobster -> TCP (vm_vsock_port) -> [vfkit port forward] -> bisque relay
//!                                                              (host_relay_port)
//!                                                                    |
//!                                                           allowlist check
//!                                                                    |
//!                                                          TCP -> remote Lobster
//! ```
//!
//! ## vfkit Port Forwarding
//!
//! To expose a VM TCP port to the host, configure vfkit with:
//!
//! ```
//! --device virtio-net,nat,portForwards=<vm_port>:<host_port>
//! ```
//!
//! Example: `--device virtio-net,nat,portForwards=3100:3101`
//!
//! This maps VM port 3100 (where the VM Lobster connects) to host port 3101
//! (where the bisque relay listens). The relay then proxies approved messages
//! to the configured remote Lobster endpoints.
//!
//! ## Security
//!
//! - Only URLs explicitly listed in `RemoteChannelConfig::remote_lobster_urls`
//!   are accepted as destinations.
//! - Every relayed message is logged with source, destination, and payload size.
//! - Connections to unlisted destinations are rejected and logged.

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(target_os = "macos"))]
pub use stub::*;

// ---------------------------------------------------------------------------
// Shared types (available on all platforms)
// ---------------------------------------------------------------------------

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A message relayed through the bisque remote channel.
///
/// Every message is logged before proxying, capturing source, destination,
/// payload size, and timestamp for auditing and debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteMessage {
    /// The originating endpoint (e.g., "vm-lobster" or a VM address).
    pub source: String,
    /// The target remote Lobster URL (must be in the allowlist).
    pub destination: String,
    /// The JSON payload to forward.
    pub payload: serde_json::Value,
    /// UTC timestamp when the message was received by the relay.
    pub timestamp: DateTime<Utc>,
}

impl RemoteMessage {
    /// Construct a new `RemoteMessage` with the current UTC timestamp.
    pub fn new(source: String, destination: String, payload: serde_json::Value) -> Self {
        Self {
            source,
            destination,
            payload,
            timestamp: Utc::now(),
        }
    }

    /// Size of the serialized payload in bytes.
    pub fn payload_size_bytes(&self) -> usize {
        self.payload.to_string().len()
    }
}

/// Configuration for the remote channel relay.
///
/// The relay listens on `host_relay_port` (TCP) for connections arriving from
/// the VM (forwarded via vfkit's `virtio-net` port-forward from
/// `vm_vsock_port`). Approved connections are proxied to one of the URLs in
/// `remote_lobster_urls`.
#[derive(Debug, Clone)]
pub struct RemoteChannelConfig {
    /// The TCP port inside the VM on which the VM Lobster connects.
    ///
    /// This is the *source* port in the vfkit port-forward rule:
    /// `--device virtio-net,nat,portForwards=<vm_vsock_port>:<host_relay_port>`
    pub vm_vsock_port: u32,

    /// The TCP port on the host where the bisque relay listens.
    ///
    /// This is the *destination* port in the vfkit port-forward rule.
    pub host_relay_port: u16,

    /// Allowlist of remote Lobster endpoints.
    ///
    /// Connections requesting a destination not in this list are rejected.
    /// Each entry should be a full URL, e.g. `"http://remote-lobster:8080"`.
    pub remote_lobster_urls: Vec<String>,
}

// ---------------------------------------------------------------------------
// macOS implementation
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos {
    use super::{RemoteChannelConfig, RemoteMessage};

    use std::sync::Arc;

    use anyhow::{anyhow, Context, Result};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::task::JoinHandle;
    use tracing::{error, info, warn};

    /// Start the TCP relay on the host.
    ///
    /// Listens on `config.host_relay_port` for connections originating from
    /// within the VM (forwarded via vfkit port-forwarding). Each accepted
    /// connection reads a newline-delimited JSON `RemoteMessage` frame,
    /// validates the destination against the allowlist, logs the message, and
    /// proxies the payload to the target remote Lobster endpoint.
    ///
    /// Returns a `JoinHandle` for the relay task. Drop or abort the handle to
    /// stop the relay.
    pub async fn start_relay(config: RemoteChannelConfig) -> Result<JoinHandle<()>> {
        let bind_addr = format!("127.0.0.1:{}", config.host_relay_port);
        let listener = TcpListener::bind(&bind_addr)
            .await
            .with_context(|| format!("Failed to bind relay listener on {}", bind_addr))?;

        info!(
            host_relay_port = config.host_relay_port,
            vm_vsock_port = config.vm_vsock_port,
            remote_urls = ?config.remote_lobster_urls,
            "Remote channel relay started"
        );

        // Share the allowlist across connection handler tasks.
        let allowlist: Arc<Vec<String>> = Arc::new(config.remote_lobster_urls);

        let handle = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        info!(peer = %peer_addr, "Accepted connection from VM");
                        let allowlist = Arc::clone(&allowlist);
                        tokio::spawn(handle_connection(stream, peer_addr.to_string(), allowlist));
                    }
                    Err(e) => {
                        error!(error = %e, "Relay listener accept error");
                        // Brief pause to avoid a tight error loop.
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Handle a single inbound connection from the VM.
    ///
    /// Reads a newline-delimited JSON `RemoteMessage`, validates the destination
    /// against the allowlist, logs the message, and forwards the payload over a
    /// new TCP connection to the remote Lobster endpoint.
    async fn handle_connection(
        mut stream: TcpStream,
        peer_addr: String,
        allowlist: Arc<Vec<String>>,
    ) {
        let mut buf = Vec::new();
        // Read until newline delimiter (max 1 MiB to prevent unbounded growth).
        let mut byte = [0u8; 1];
        loop {
            match stream.read(&mut byte).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    if byte[0] == b'\n' {
                        break;
                    }
                    buf.push(byte[0]);
                    if buf.len() > 1_048_576 {
                        warn!(peer = %peer_addr, "Message exceeds 1 MiB limit, dropping connection");
                        return;
                    }
                }
                Err(e) => {
                    error!(peer = %peer_addr, error = %e, "Read error from VM connection");
                    return;
                }
            }
        }

        // Deserialise the RemoteMessage envelope.
        let message: RemoteMessage = match serde_json::from_slice(&buf) {
            Ok(m) => m,
            Err(e) => {
                warn!(peer = %peer_addr, error = %e, "Failed to parse RemoteMessage from VM");
                let _ = stream
                    .write_all(b"{\"error\":\"invalid_message\"}\n")
                    .await;
                return;
            }
        };

        // Validate destination against the allowlist.
        if !allowlist.contains(&message.destination) {
            warn!(
                peer = %peer_addr,
                destination = %message.destination,
                "Rejected: destination not in allowlist"
            );
            let _ = stream
                .write_all(b"{\"error\":\"destination_not_allowed\"}\n")
                .await;
            return;
        }

        // Log the relayed message (source, destination, payload size).
        log_message(&message);

        // Forward the payload to the remote Lobster endpoint.
        match proxy_to_remote(&message).await {
            Ok(response) => {
                let mut resp_bytes = response;
                resp_bytes.push(b'\n');
                let _ = stream.write_all(&resp_bytes).await;
            }
            Err(e) => {
                error!(
                    source = %message.source,
                    destination = %message.destination,
                    error = %e,
                    "Failed to proxy message to remote Lobster"
                );
                let _ = stream
                    .write_all(b"{\"error\":\"proxy_failed\"}\n")
                    .await;
            }
        }
    }

    /// Log a relayed message with source, destination, and payload size.
    fn log_message(message: &RemoteMessage) {
        info!(
            source = %message.source,
            destination = %message.destination,
            payload_bytes = message.payload_size_bytes(),
            timestamp = %message.timestamp,
            "Relaying message to remote Lobster"
        );
    }

    /// Forward a `RemoteMessage` payload to the remote Lobster endpoint via TCP.
    ///
    /// Opens a new TCP connection to the destination, sends the JSON payload
    /// (newline-delimited), and reads back the response.
    ///
    /// The destination URL is expected to be in the form `"host:port"` or a
    /// full `"http://host:port"` URL. This function strips any `http://` or
    /// `https://` scheme prefix before connecting.
    async fn proxy_to_remote(message: &RemoteMessage) -> anyhow::Result<Vec<u8>> {
        let dest = strip_scheme(&message.destination);

        let mut remote = TcpStream::connect(dest)
            .await
            .with_context(|| format!("TCP connect to remote Lobster at {} failed", dest))?;

        let payload_bytes = serde_json::to_vec(&message.payload)
            .context("Failed to serialise payload")?;
        remote
            .write_all(&payload_bytes)
            .await
            .context("Failed to write payload to remote")?;
        remote
            .write_all(b"\n")
            .await
            .context("Failed to write newline delimiter")?;

        let mut response = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match remote.read(&mut byte).await {
                Ok(0) => break,
                Ok(_) => {
                    if byte[0] == b'\n' {
                        break;
                    }
                    response.push(byte[0]);
                    if response.len() > 1_048_576 {
                        return Err(anyhow!("Remote response exceeds 1 MiB limit"));
                    }
                }
                Err(e) => return Err(anyhow!("Read error from remote: {}", e)),
            }
        }

        Ok(response)
    }

    /// Strip an `http://` or `https://` scheme prefix from a URL, returning
    /// just the `host:port` portion suitable for `TcpStream::connect`.
    fn strip_scheme(url: &str) -> &str {
        url.strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn strip_scheme_removes_http() {
            assert_eq!(strip_scheme("http://remote-lobster:8080"), "remote-lobster:8080");
        }

        #[test]
        fn strip_scheme_removes_https() {
            assert_eq!(strip_scheme("https://remote-lobster:8080"), "remote-lobster:8080");
        }

        #[test]
        fn strip_scheme_passthrough_bare_host() {
            assert_eq!(strip_scheme("remote-lobster:8080"), "remote-lobster:8080");
        }

        #[test]
        fn remote_message_payload_size() {
            let msg = RemoteMessage::new(
                "vm-lobster".to_string(),
                "http://remote:9000".to_string(),
                serde_json::json!({"hello": "world"}),
            );
            assert_eq!(msg.payload_size_bytes(), 17);
        }

        #[test]
        fn remote_message_construction() {
            let payload = serde_json::json!({"key": "value"});
            let msg = RemoteMessage::new(
                "source".to_string(),
                "dest".to_string(),
                payload.clone(),
            );
            assert_eq!(msg.source, "source");
            assert_eq!(msg.destination, "dest");
            assert_eq!(msg.payload, payload);
        }
    }
}

// ---------------------------------------------------------------------------
// Linux / non-macOS stub
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "macos"))]
mod stub {
    use super::RemoteChannelConfig;

    use anyhow::Result;
    use tokio::task::JoinHandle;
    use tracing::warn;

    /// No-op relay for non-macOS platforms.
    ///
    /// The remote channel is a macOS-only feature (tied to the vfkit
    /// virtualization stack). On Linux and other platforms this function logs a
    /// warning and returns a task that completes immediately.
    pub async fn start_relay(config: RemoteChannelConfig) -> Result<JoinHandle<()>> {
        warn!(
            host_relay_port = config.host_relay_port,
            "Remote channel relay is not supported on this platform (macOS only)"
        );
        Ok(tokio::spawn(async {}))
    }
}
