//! Remote Lobster communication channel via TCP relay.
//!
//! The VM has no direct internet access — all outbound communication flows
//! through this host-side relay, which enforces an allowlist of permitted
//! remote Lobster endpoints.

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(target_os = "macos"))]
pub use stub::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteMessage {
    pub source: String,
    pub destination: String,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

impl RemoteMessage {
    pub fn new(source: String, destination: String, payload: serde_json::Value) -> Self {
        Self { source, destination, payload, timestamp: Utc::now() }
    }

    pub fn payload_size_bytes(&self) -> usize {
        self.payload.to_string().len()
    }
}

#[derive(Debug, Clone)]
pub struct RemoteChannelConfig {
    pub vm_vsock_port: u32,
    pub host_relay_port: u16,
    pub remote_lobster_urls: Vec<String>,
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{RemoteChannelConfig, RemoteMessage};
    use std::sync::Arc;
    use anyhow::{anyhow, Context, Result};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::task::JoinHandle;
    use tracing::{error, info, warn};

    pub async fn start_relay(config: RemoteChannelConfig) -> Result<JoinHandle<()>> {
        let bind_addr = format!("127.0.0.1:{}", config.host_relay_port);
        let listener = TcpListener::bind(&bind_addr)
            .await
            .with_context(|| format!("Failed to bind relay listener on {}", bind_addr))?;
        info!(host_relay_port = config.host_relay_port, "Remote channel relay started");
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
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        Ok(handle)
    }

    async fn handle_connection(
        mut stream: TcpStream,
        peer_addr: String,
        allowlist: Arc<Vec<String>>,
    ) {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match stream.read(&mut byte).await {
                Ok(0) => break,
                Ok(_) => {
                    if byte[0] == b'\n' { break; }
                    buf.push(byte[0]);
                    if buf.len() > 1_048_576 {
                        warn!(peer = %peer_addr, "Message exceeds 1 MiB limit");
                        return;
                    }
                }
                Err(e) => { error!(peer = %peer_addr, error = %e, "Read error"); return; }
            }
        }
        let message: RemoteMessage = match serde_json::from_slice(&buf) {
            Ok(m) => m,
            Err(e) => {
                warn!(peer = %peer_addr, error = %e, "Failed to parse RemoteMessage");
                let _ = stream.write_all(b"{\"error\":\"invalid_message\"}\n").await;
                return;
            }
        };
        if !allowlist.contains(&message.destination) {
            warn!(peer = %peer_addr, destination = %message.destination, "Rejected: not in allowlist");
            let _ = stream.write_all(b"{\"error\":\"destination_not_allowed\"}\n").await;
            return;
        }
        info!(source = %message.source, destination = %message.destination,
              payload_bytes = message.payload_size_bytes(), "Relaying message");
        match proxy_to_remote(&message).await {
            Ok(mut response) => { response.push(b'\n'); let _ = stream.write_all(&response).await; }
            Err(e) => {
                error!(error = %e, "Failed to proxy message");
                let _ = stream.write_all(b"{\"error\":\"proxy_failed\"}\n").await;
            }
        }
    }

    async fn proxy_to_remote(message: &RemoteMessage) -> anyhow::Result<Vec<u8>> {
        let dest = strip_scheme(&message.destination);
        let mut remote = TcpStream::connect(&dest)
            .await
            .with_context(|| format!("TCP connect to {} failed", dest))?;
        let payload_bytes = serde_json::to_vec(&message.payload).context("serialise payload")?;
        remote.write_all(&payload_bytes).await.context("write payload")?;
        remote.write_all(b"\n").await.context("write newline")?;
        let mut response = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match remote.read(&mut byte).await {
                Ok(0) => break,
                Ok(_) => {
                    if byte[0] == b'\n' { break; }
                    response.push(byte[0]);
                    if response.len() > 1_048_576 { return Err(anyhow!("Response exceeds 1 MiB")); }
                }
                Err(e) => return Err(anyhow!("Read error: {}", e)),
            }
        }
        Ok(response)
    }

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
        fn remote_message_payload_size() {
            let msg = RemoteMessage::new("src".into(), "dst".into(), serde_json::json!({"hello": "world"}));
            assert_eq!(msg.payload_size_bytes(), 17);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod stub {
    use super::RemoteChannelConfig;
    use anyhow::Result;
    use tokio::task::JoinHandle;
    use tracing::warn;

    pub async fn start_relay(config: RemoteChannelConfig) -> Result<JoinHandle<()>> {
        warn!(host_relay_port = config.host_relay_port,
              "Remote channel relay is not supported on this platform (macOS only)");
        Ok(tokio::spawn(async {}))
    }
}
