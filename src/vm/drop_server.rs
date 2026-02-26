//! Drop folder server: HTTP endpoint + FSEvents watcher for VM file ingestion.
//!
//! Two complementary ingestion paths let users get files into the Lobster VM:
//!
//! **Path 1 — HTTP upload (programmatic):**
//! ```text
//! POST http://127.0.0.1:7788/drop  (multipart/form-data)
//!   → saves file to host_drop_folder/
//!   → virtio-fs propagates it into the VM automatically
//! ```
//!
//! **Path 2 — Finder drag (FSEvents):**
//! ```text
//! User drags file → Finder copies it to host_drop_folder/
//!   → notify crate fires (FSEvents backend on macOS)
//!   → bisque logs the event + writes .pending notification JSON
//!   → VM's Lobster instance reads .pending and processes the file
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::routing::post;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct DropServerConfig {
    pub host_drop_folder: PathBuf,
    pub vm_drop_folder_path: PathBuf,
    pub http_port: u16,
}

impl Default for DropServerConfig {
    fn default() -> Self {
        Self {
            host_drop_folder: default_host_drop_folder(),
            vm_drop_folder_path: PathBuf::from("/mnt/lobster-drop"),
            http_port: 7788,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropEvent {
    pub filename: String,
    pub size_bytes: u64,
    pub timestamp: DateTime<Utc>,
    pub destination_path_in_vm: PathBuf,
}

pub async fn start_drop_server(
    config: DropServerConfig,
) -> Result<(JoinHandle<()>, broadcast::Receiver<DropEvent>)> {
    tokio::fs::create_dir_all(&config.host_drop_folder)
        .await
        .with_context(|| format!("failed to create host drop folder: {}", config.host_drop_folder.display()))?;

    let (tx, rx) = broadcast::channel::<DropEvent>(64);
    let config = Arc::new(config);

    let http_handle = spawn_http_server(Arc::clone(&config), tx.clone()).await?;
    let watcher_handle = spawn_fs_watcher(Arc::clone(&config), tx)?;

    let supervisor = tokio::spawn(async move {
        tokio::select! {
            res = http_handle => {
                match res {
                    Ok(()) => info!("drop server HTTP task exited"),
                    Err(e) => error!("drop server HTTP task panicked: {e}"),
                }
            }
            res = watcher_handle => {
                match res {
                    Ok(()) => info!("drop server FS watcher task exited"),
                    Err(e) => error!("drop server FS watcher task panicked: {e}"),
                }
            }
        }
    });

    Ok((supervisor, rx))
}

#[derive(Clone)]
struct ServerState {
    config: Arc<DropServerConfig>,
    tx: broadcast::Sender<DropEvent>,
}

async fn spawn_http_server(
    config: Arc<DropServerConfig>,
    tx: broadcast::Sender<DropEvent>,
) -> Result<JoinHandle<()>> {
    let addr = format!("127.0.0.1:{}", config.http_port);
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind HTTP server to {addr}"))?;
    info!("drop server listening on http://{addr}");
    let state = ServerState { config, tx };
    let app = Router::new().route("/drop", post(handle_drop)).with_state(state);
    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("drop server HTTP error: {e}");
        }
    });
    Ok(handle)
}

async fn handle_drop(
    State(state): State<ServerState>,
    mut multipart: Multipart,
) -> (StatusCode, String) {
    let mut saved = Vec::new();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return (StatusCode::BAD_REQUEST, format!("multipart error: {e}")),
        };
        let filename = match field.file_name().map(|s| s.to_owned()) {
            Some(name) if !name.is_empty() => sanitize_filename(&name),
            _ => continue,
        };
        let data = match field.bytes().await {
            Ok(b) => b,
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("read error: {e}")),
        };
        let dest = state.config.host_drop_folder.join(&filename);
        let tmp = dest.with_extension("tmp");
        if let Err(e) = tokio::fs::write(&tmp, &data).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("write error: {e}"));
        }
        if let Err(e) = tokio::fs::rename(&tmp, &dest).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("rename error: {e}"));
        }
        let size_bytes = data.len() as u64;
        let event = DropEvent {
            filename: filename.clone(),
            size_bytes,
            timestamp: Utc::now(),
            destination_path_in_vm: state.config.vm_drop_folder_path.join(&filename),
        };
        info!(filename = %filename, size_bytes, "HTTP drop: file saved");
        write_pending_notification(&state.config, &event).await;
        let _ = state.tx.send(event);
        saved.push(filename);
    }
    if saved.is_empty() {
        (StatusCode::BAD_REQUEST, "no files received".to_string())
    } else {
        (StatusCode::OK, format!("saved: {}", saved.join(", ")))
    }
}

fn spawn_fs_watcher(
    config: Arc<DropServerConfig>,
    tx: broadcast::Sender<DropEvent>,
) -> Result<JoinHandle<()>> {
    use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;

    let watch_path = config.host_drop_folder.clone();
    let (fs_tx, fs_rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = RecommendedWatcher::new(fs_tx, NotifyConfig::default())
        .context("failed to create FSEvents watcher")?;
    watcher.watch(&watch_path, RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch {}", watch_path.display()))?;
    info!(path = %watch_path.display(), "drop folder FSEvents watcher started");

    let handle = tokio::task::spawn_blocking(move || {
        let _watcher = watcher;
        for result in fs_rx {
            match result {
                Ok(event) => handle_fs_event(&config, &tx, event),
                Err(e) => error!("FSEvents watcher error: {e}"),
            }
        }
        info!("FSEvents watcher thread exiting");
    });
    Ok(handle)
}

fn handle_fs_event(
    config: &DropServerConfig,
    tx: &broadcast::Sender<DropEvent>,
    event: notify::Event,
) {
    use notify::EventKind;
    let is_create = matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(notify::event::ModifyKind::Name(_))
    );
    if !is_create { return; }

    for path in &event.paths {
        if path.is_dir() { continue; }
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_owned(),
            None => continue,
        };
        if filename.starts_with('.') || filename.ends_with(".tmp") { continue; }
        let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let drop_event = DropEvent {
            filename: filename.clone(),
            size_bytes,
            timestamp: Utc::now(),
            destination_path_in_vm: config.vm_drop_folder_path.join(&filename),
        };
        info!(filename = %filename, size_bytes, "new file in drop folder");
        let config_clone = config.clone();
        let event_clone = drop_event.clone();
        tokio::task::spawn(async move {
            write_pending_notification(&config_clone, &event_clone).await;
        });
        if let Err(e) = tx.send(drop_event) {
            warn!("no broadcast receivers for DropEvent: {e}");
        }
    }
}

async fn write_pending_notification(config: &DropServerConfig, event: &DropEvent) {
    let pending_path = config.host_drop_folder.join(".pending");
    let mut entries: Vec<PendingEntry> = match tokio::fs::read(&pending_path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    entries.push(PendingEntry {
        filename: event.filename.clone(),
        size_bytes: event.size_bytes,
        arrived_at: event.timestamp,
    });
    match serde_json::to_vec_pretty(&entries) {
        Ok(json) => { if let Err(e) = tokio::fs::write(&pending_path, json).await { warn!("failed to write .pending: {e}"); } }
        Err(e) => warn!("failed to serialise .pending: {e}"),
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PendingEntry {
    filename: String,
    size_bytes: u64,
    arrived_at: DateTime<Utc>,
}

fn default_host_drop_folder() -> PathBuf {
    let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/tmp"));
    #[cfg(target_os = "macos")]
    { home.join("Library").join("Application Support").join("com.fullyparsed.bisque-computer").join("lobster-drop") }
    #[cfg(not(target_os = "macos"))]
    { home.join(".local").join("share").join("bisque-computer").join("lobster-drop") }
}

fn sanitize_filename(raw: &str) -> String {
    raw.chars()
        .filter(|&c| c != '/' && c != '\\' && c != '\0')
        .collect::<String>()
        .trim_start_matches('.')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_blocks_path_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "etc/passwd");
        assert_eq!(sanitize_filename("foo/bar.txt"), "foobar.txt");
        assert_eq!(sanitize_filename("normal.txt"), "normal.txt");
    }

    #[test]
    fn sanitize_strips_leading_dot() {
        assert_eq!(sanitize_filename(".hidden"), "hidden");
    }

    #[test]
    fn drop_event_serialises() {
        let event = DropEvent {
            filename: "test.txt".to_owned(),
            size_bytes: 42,
            timestamp: Utc::now(),
            destination_path_in_vm: PathBuf::from("/mnt/lobster-drop/test.txt"),
        };
        let json = serde_json::to_string(&event).expect("serialise");
        let back: DropEvent = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back.filename, "test.txt");
        assert_eq!(back.size_bytes, 42);
    }

    #[test]
    fn default_config_has_correct_port() {
        let cfg = DropServerConfig::default();
        assert_eq!(cfg.http_port, 7788);
    }
}
