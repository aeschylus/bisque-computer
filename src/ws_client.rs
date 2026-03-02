//! WebSocket client for connecting to Lobster Dashboard servers.
//!
//! Runs in a Tokio background task and pushes state updates into
//! a shared structure that the rendering loop reads from.
//!
//! Outbound messages (e.g., voice_input) are queued via `OutboundSender`
//! and broadcast to all connected instances via per-client mpsc channels.
//!
//! # RTT tracking
//!
//! Every `PING_INTERVAL_SECS` seconds the client sends an application-level
//! ping: `{"type":"ping","sent_at_ms":<unix_ms>}`. When the server echoes it
//! back as a pong the elapsed time is fed into a Mosh-compatible EWMA filter
//! (alpha = 0.2) and stored in `LobsterInstance::rtt_ms`. The render loop
//! reads this value each frame and forwards it to every `PredictionEngine`
//! via `PaneTree::set_rtt_all()`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::protocol::{ConnectionStatus, DashboardState, Frame, LobsterInstance};

/// Send an application-level ping every this many seconds.
const PING_INTERVAL_SECS: u64 = 2;

/// EWMA smoothing factor for RTT, matching Mosh's SRTT implementation.
const EWMA_ALPHA: f64 = 0.2;

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

/// Returns the current time as milliseconds since the Unix epoch.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}

/// Serialise a JSON ping payload without pulling in serde overhead.
fn make_ping_json(sent_at_ms: u64) -> String {
    format!(r#"{{"type":"ping","sent_at_ms":{}}}"#, sent_at_ms)
}

/// Reconnecting client loop for a single Lobster instance.
///
/// Accepts outbound messages via `outbound_rx` and forwards them to the server
/// when connected. Periodically sends application-level pings and computes a
/// smoothed RTT estimate via EWMA.
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

                let (mut write, mut read) = ws_stream.split();

                // Per-connection RTT state — lives entirely inside this task,
                // so no synchronisation is needed beyond writing the final u32
                // into the shared LobsterInstance.
                let mut ping_ticker = interval(Duration::from_secs(PING_INTERVAL_SECS));
                let mut smoothed_rtt: f64 = 0.0;
                let mut last_ping_sent_at: Option<u64> = None;

                loop {
                    tokio::select! {
                        // Inbound: messages from the server
                        msg_result = read.next() => {
                            match msg_result {
                                Some(Ok(Message::Text(text))) => {
                                    handle_message(
                                        &shared,
                                        index,
                                        &text,
                                        last_ping_sent_at,
                                        &mut smoothed_rtt,
                                    );
                                }
                                Some(Ok(Message::Ping(data))) => {
                                    let _ = write.send(Message::Pong(data)).await;
                                }
                                Some(Ok(Message::Close(_))) | None => break,
                                Some(Err(e)) => {
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
                                eprintln!("Failed to send outbound message: {}", e);
                                break;
                            }
                        }

                        // Periodic ping for RTT measurement
                        _ = ping_ticker.tick() => {
                            let sent_at_ms = now_ms();
                            last_ping_sent_at = Some(sent_at_ms);
                            let ping_json = make_ping_json(sent_at_ms);
                            let msg = Message::Text(ping_json.into());
                            if let Err(e) = write.send(msg).await {
                                eprintln!("Failed to send ping: {}", e);
                                break;
                            }
                        }
                    }
                }

                // Connection closed — reset RTT so stale values don't activate
                // prediction on the next re-connect while still disconnected.
                {
                    let mut instances = shared.lock().unwrap();
                    if let Some(inst) = instances.get_mut(index) {
                        if inst.status == ConnectionStatus::Connected {
                            inst.status = ConnectionStatus::Disconnected;
                        }
                        inst.rtt_ms = 0;
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
///
/// `last_ping_sent_at` is the millisecond timestamp embedded in the most
/// recently sent ping. `smoothed_rtt` is the per-connection EWMA accumulator
/// (owned by `client_loop`); on a matching pong it is updated in-place and
/// the rounded value is stored in `inst.rtt_ms`.
fn handle_message(
    shared: &SharedInstances,
    index: usize,
    text: &str,
    last_ping_sent_at: Option<u64>,
    smoothed_rtt: &mut f64,
) {
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
            // Extract the echoed sent_at_ms from the pong payload and compute
            // a new EWMA sample only when it matches the outstanding ping.
            let echoed_sent_at = frame
                .data
                .as_ref()
                .and_then(|d| d.get("sent_at_ms"))
                .and_then(|v| v.as_u64());

            if let (Some(echoed_ms), Some(sent_at_ms)) = (echoed_sent_at, last_ping_sent_at) {
                if echoed_ms == sent_at_ms {
                    let sample_rtt = now_ms().saturating_sub(echoed_ms) as f64;
                    // Cold-start: seed with first sample rather than blending
                    // from 0, which would underestimate for several rounds.
                    if *smoothed_rtt == 0.0 {
                        *smoothed_rtt = sample_rtt;
                    } else {
                        *smoothed_rtt =
                            (1.0 - EWMA_ALPHA) * (*smoothed_rtt) + EWMA_ALPHA * sample_rtt;
                    }
                    inst.rtt_ms = (*smoothed_rtt).round() as u32;
                }
            }
        }
        _ => {}
    }
}
