//! WebSocket client for connecting to Lobster Dashboard servers.
//!
//! Runs in a Tokio background task and pushes state updates into
//! a shared structure that the rendering loop reads from.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::protocol::{ConnectionStatus, DashboardState, Frame, LobsterInstance};

/// Shared state accessible from both the WebSocket tasks and the render loop.
pub type SharedInstances = Arc<Mutex<Vec<LobsterInstance>>>;

/// Spawn WebSocket client tasks for each endpoint URL.
/// Returns the shared instances handle.
pub fn spawn_clients(
    runtime: &tokio::runtime::Runtime,
    urls: Vec<String>,
) -> SharedInstances {
    let instances: Vec<LobsterInstance> = urls
        .iter()
        .map(|u| LobsterInstance::new(u.clone()))
        .collect();
    let shared = Arc::new(Mutex::new(instances));

    for (index, url) in urls.into_iter().enumerate() {
        let shared_clone = Arc::clone(&shared);
        runtime.spawn(client_loop(shared_clone, index, url));
    }

    shared
}

/// Reconnecting client loop for a single Lobster instance.
async fn client_loop(shared: SharedInstances, index: usize, url: String) {
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

                let (mut write, mut read) = ws_stream.split();

                // Read messages from the server
                while let Some(msg_result) = read.next().await {
                    match msg_result {
                        Ok(Message::Text(text)) => {
                            handle_message(&shared, index, &text);
                        }
                        Ok(Message::Ping(data)) => {
                            let _ = write.send(Message::Pong(data)).await;
                        }
                        Ok(Message::Close(_)) => {
                            break;
                        }
                        Err(e) => {
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
