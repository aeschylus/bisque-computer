//! WebSocket client for connecting to Lobster Dashboard servers.
//!
//! Runs in a Tokio background task and pushes state updates into
//! a shared structure that the rendering loop reads from.
//!
//! Outbound messages (e.g., voice_input) are queued via `OutboundSender`
//! and broadcast to all connected instances via per-client mpsc channels.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

use crate::protocol::{ConnectionStatus, DashboardState, Frame, LobsterInstance};

/// Shared state accessible from both the WebSocket tasks and the render loop.
pub type SharedInstances = Arc<Mutex<Vec<LobsterInstance>>>;

/// A cloneable handle for sending outbound JSON messages to all connected
/// WebSocket instances. Each send is broadcast to every connected client.
#[derive(Clone)]
pub struct OutboundSender {
    /// One sender per registered client instance.
    senders: Arc<Vec<mpsc::UnboundedSender<String>>>,
}

impl OutboundSender {
    fn new(senders: Vec<mpsc::UnboundedSender<String>>) -> Self {
        Self {
            senders: Arc::new(senders),
        }
    }

    /// Broadcast a JSON payload to all connected instances.
    ///
    /// Silently drops sends to disconnected clients (their receivers were dropped).
    pub fn broadcast(&self, json: String) {
        for sender in self.senders.iter() {
            let _ = sender.send(json.clone());
        }
    }
}

/// Spawn WebSocket client tasks for each endpoint URL.
///
/// Returns:
/// - `SharedInstances`: live connection state used by the render loop.
/// - `OutboundSender`: broadcasts JSON messages to all connected instances.
pub fn spawn_clients(
    runtime: &tokio::runtime::Runtime,
    urls: Vec<String>,
) -> (SharedInstances, OutboundSender) {
    let instances: Vec<LobsterInstance> = urls
        .iter()
        .map(|u| LobsterInstance::new(u.clone()))
        .collect();
    let shared = Arc::new(Mutex::new(instances));

    // Build one mpsc channel per client instance for outbound fan-out.
    let mut per_client_senders: Vec<mpsc::UnboundedSender<String>> = Vec::new();

    for (index, url) in urls.into_iter().enumerate() {
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        per_client_senders.push(tx);

        let shared_clone = Arc::clone(&shared);
        runtime.spawn(client_loop(shared_clone, index, url, rx));
    }

    let outbound = OutboundSender::new(per_client_senders);
    (shared, outbound)
}

/// Reconnecting client loop for a single Lobster instance.
///
/// Accepts outbound messages via `outbound_rx` and forwards them to the server
/// when connected.
async fn client_loop(
    shared: SharedInstances,
    index: usize,
    url: String,
    mut outbound_rx: mpsc::UnboundedReceiver<String>,
) {
    loop {
        // Update status to Connecting
        {
            let mut instances = shared.lock().unwrap();
            if let Some(inst) = instances.get_mut(index) {
                inst.status = ConnectionStatus::Connecting;
            }
        }

        match connect_async(&url).await {
            Ok((ws_stream, _response)) => {
                // Update status to Connected
                {
                    let mut instances = shared.lock().unwrap();
                    if let Some(inst) = instances.get_mut(index) {
                        inst.status = ConnectionStatus::Connected;
                    }
                }
                info!(target: "ws_client", url = %url, "WebSocket connected");

                let (mut write, mut read) = ws_stream.split();

                loop {
                    tokio::select! {
                        // Inbound: messages from the server
                        msg_result = read.next() => {
                            match msg_result {
                                Some(Ok(Message::Text(text))) => {
                                    handle_message(&shared, index, &text);
                                }
                                Some(Ok(Message::Ping(data))) => {
                                    let _ = write.send(Message::Pong(data)).await;
                                }
                                Some(Ok(Message::Close(_))) | None => {
                                    info!(target: "ws_client", url = %url, "WebSocket closed by server");
                                    break;
                                }
                                Some(Err(e)) => {
                                    error!(target: "ws_client", url = %url, "WebSocket error: {}", e);
                                    let mut instances = shared.lock().unwrap();
                                    if let Some(inst) = instances.get_mut(index) {
                                        inst.status =
                                            ConnectionStatus::Error(format!("WS error: {}", e));
                                    }
                                    break;
                                }
                                _ => {}
                            }
                        }

                        // Outbound: messages queued by voice input or other features
                        Some(json) = outbound_rx.recv() => {
                            let msg = Message::Text(json.into());
                            if let Err(e) = write.send(msg).await {
                                error!(target: "ws_client", "Failed to send outbound message: {}", e);
                                break;
                            }
                        }
                    }
                }

                // Connection closed
                {
                    let mut instances = shared.lock().unwrap();
                    if let Some(inst) = instances.get_mut(index) {
                        if inst.status == ConnectionStatus::Connected {
                            inst.status = ConnectionStatus::Disconnected;
                        }
                    }
                }
            }
            Err(e) => {
                warn!(target: "ws_client", url = %url, "Connection failed: {} — retrying in 3s", e);
                let mut instances = shared.lock().unwrap();
                if let Some(inst) = instances.get_mut(index) {
                    inst.status = ConnectionStatus::Error(format!("Connect failed: {}", e));
                }
            }
        }

        // Wait before reconnecting
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

/// Parse a server message and update the shared instance state.
fn handle_message(shared: &SharedInstances, index: usize, text: &str) {
    let frame: Frame = match serde_json::from_str(text) {
        Ok(f) => f,
        Err(_) => return,
    };

    let mut instances = shared.lock().unwrap();
    let inst = match instances.get_mut(index) {
        Some(i) => i,
        None => return,
    };

    match frame.msg_type.as_str() {
        "hello" => {
            if let Some(data) = &frame.data {
                if let Some(pv) = data.get("protocol_version").and_then(|v| v.as_str()) {
                    inst.protocol_version = Some(pv.to_string());
                }
            }
        }
        "snapshot" | "update" => {
            if let Some(data) = frame.data {
                if let Ok(state) = serde_json::from_value::<DashboardState>(data) {
                    inst.state = state;
                    inst.last_update = Some(frame.timestamp);
                }
            }
        }
        "pong" => {
            // Could track latency here
        }
        _ => {}
    }
}
