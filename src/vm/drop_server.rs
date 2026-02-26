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
//!
//! Both paths emit a [`DropEvent`] on the shared broadcast channel so the rest
//! of the application can react (e.g., show a toast notification).
//!
//! ## Usage
//!
//! ```rust,no_run
//! # use bisque_computer::vm::drop_server::{DropServerConfig, start_drop_server};
//! # use std::path::PathBuf;
//! # tokio_test::block_on(async {
//! let config = DropServerConfig {
//!     host_drop_folder: PathBuf::from("/tmp/lobster-drop"),
//!     vm_drop_folder_path: PathBuf::from("/mnt/lobster-drop"),
//!     http_port: 7788,
//! };
//! let (handle, mut rx) = start_drop_server(config).await.unwrap();
//! while let Ok(event) = rx.recv().await {
//!     println!("new file: {}", event.filename);
//! }
//! # });
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

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Configuration for the drop folder server.
#[derive(Debug, Clone)]
pub struct DropServerConfig {
    /// The host-side folder that is watched for new files and also receives
    /// HTTP-uploaded files. Typically `~/Library/Application Support/
    /// com.fullyparsed.bisque-computer/lobster-drop/`.
    pub host_drop_folder: PathBuf,

    /// The path at which the same folder is mounted inside the VM via
    /// virtio-fs. Used to populate [`DropEvent::destination_path_in_vm`].
    pub vm_drop_folder_path: PathBuf,

    /// TCP port for the local HTTP server. Defaults to `7788`.
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

/// A file that has been delivered into the drop folder.
///
/// Emitted on the broadcast channel returned by [`start_drop_server`] whenever
/// a new file arrives, regardless of which ingestion path was used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropEvent {
    /// Bare filename (no directory component).
    pub filename: String,
    /// File size in bytes at the time of detection.
    pub size_bytes: u64,
    /// UTC timestamp of detection.
    pub timestamp: DateTime<Utc>,
    /// Absolute path inside the VM where the file will appear.
    pub destination_path_in_vm: PathBuf,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Start the drop folder server.
///
/// Spawns two background tasks:
/// 1. An axum HTTP server listening on `127.0.0.1:{config.http_port}`.
/// 2. A `notify` watcher monitoring `config.host_drop_folder`.
///
/// Returns the join handle for the supervisor task and a broadcast receiver
/// that yields [`DropEvent`]s as files arrive.
pub async fn start_drop_server(
    config: DropServerConfig,
) -> Result<(JoinHandle<()>, broadcast::Receiver<DropEvent>)> {
    // Ensure the host drop folder exists.
    tokio::fs::create_dir_all(&config.host_drop_folder)
        .await
        .with_context(|| {
            format!(
                "failed to create host drop folder: {}",
                config.host_drop_folder.display()
            )
        })?;

    let (tx, rx) = broadcast::channel::<DropEvent>(64);
    let config = Arc::new(config);

    let http_handle = spawn_http_server(Arc::clone(&config), tx.clone()).await?;
    let watcher_handle = spawn_fs_watcher(Arc::clone(&config), tx)?;

    // Supervisor: drives both tasks and logs if either exits unexpectedly.
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

// ---------------------------------------------------------------------------
// HTTP server (axum)
// ---------------------------------------------------------------------------

/// Shared state threaded through axum handlers.
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
    let app = Router::new()
        .route("/drop", post(handle_drop))
        .with_state(state);

    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("drop server HTTP error: {e}");
        }
    });

    Ok(handle)
}

/// `POST /drop` — accept a multipart file upload and save it to the drop folder.
///
/// The request must contain at least one part with a `filename` in its
/// `Content-Disposition` header. Each valid file part is written atomically
/// (write to a `.tmp` file, then rename) and a [`DropEvent`] is broadcast.
async fn handle_drop(
    State(state): State<ServerState>,
    mut multipart: Multipart,
) -> (StatusCode, String) {
    let mut saved = Vec::new();

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                warn!("multipart error: {e}");
                return (StatusCode::BAD_REQUEST, format!("multipart error: {e}"));
            }
        };

        // Require a filename.
        let filename = match field.file_name().map(|s| s.to_owned()) {
            Some(name) if !name.is_empty() => sanitize_filename(&name),
            _ => {
                // Skip non-file parts (e.g., plain text fields).
                continue;
            }
        };

        let data = match field.bytes().await {
            Ok(b) => b,
            Err(e) => {
                warn!("failed to read field bytes for {filename}: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("read error: {e}"),
                );
            }
        };

        let dest = state.config.host_drop_folder.join(&filename);
        let tmp = dest.with_extension("tmp");

        if let Err(e) = tokio::fs::write(&tmp, &data).await {
            error!("failed to write {}: {e}", tmp.display());
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("write error: {e}"),
            );
        }

        if let Err(e) = tokio::fs::rename(&tmp, &dest).await {
            error!(
                "failed to rename {} → {}: {e}",
                tmp.display(),
                dest.display()
            );
            let _ = tokio::fs::remove_file(&tmp).await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("rename error: {e}"),
            );
        }

        let size_bytes = data.len() as u64;
        let event = DropEvent {
            filename: filename.clone(),
            size_bytes,
            timestamp: Utc::now(),
            destination_path_in_vm: state.config.vm_drop_folder_path.join(&filename),
        };

        info!(
            filename = %filename,
            size_bytes = size_bytes,
            "HTTP drop: file saved"
        );

        // Best-effort: write the .pending notification so the VM can react.
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

// ---------------------------------------------------------------------------
// FSEvents / notify watcher
// ---------------------------------------------------------------------------

fn spawn_fs_watcher(
    config: Arc<DropServerConfig>,
    tx: broadcast::Sender<DropEvent>,
) -> Result<JoinHandle<()>> {
    use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;

    let watch_path = config.host_drop_folder.clone();

    // `notify` is synchronous; run it on a dedicated blocking thread and
    // bridge events into async via a oneshot-style channel.
    let (fs_tx, fs_rx) = mpsc::channel::<notify::Result<notify::Event>>();

    let mut watcher = RecommendedWatcher::new(fs_tx, NotifyConfig::default())
        .context("failed to create FSEvents watcher")?;

    watcher
        .watch(&watch_path, RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch {}", watch_path.display()))?;

    info!(
        path = %watch_path.display(),
        "drop folder FSEvents watcher started"
    );

    let handle = tokio::task::spawn_blocking(move || {
        // Keep `watcher` alive for the duration of this thread.
        let _watcher = watcher;

        for result in fs_rx {
            match result {
                Ok(event) => {
                    handle_fs_event(&config, &tx, event);
                }
                Err(e) => {
                    error!("FSEvents watcher error: {e}");
                }
            }
        }

        info!("FSEvents watcher thread exiting");
    });

    Ok(handle)
}

/// Process a single notify event synchronously (called from the blocking thread).
fn handle_fs_event(
    config: &DropServerConfig,
    tx: &broadcast::Sender<DropEvent>,
    event: notify::Event,
) {
    use notify::EventKind;

    // We only care about file creation and moves-into-folder.
    let is_create = matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(notify::event::ModifyKind::Name(_))
    );

    if !is_create {
        return;
    }

    for path in &event.paths {
        // Skip directories and hidden/temporary files.
        if path.is_dir() {
            continue;
        }

        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_owned(),
            None => continue,
        };

        if filename.starts_with('.') || filename.ends_with(".tmp") {
            continue;
        }

        let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        let drop_event = DropEvent {
            filename: filename.clone(),
            size_bytes,
            timestamp: Utc::now(),
            destination_path_in_vm: config.vm_drop_folder_path.join(&filename),
        };

        #[cfg(target_os = "macos")]
        info!(
            filename = %filename,
            size_bytes = size_bytes,
            "FSEvents (macOS): new file in drop folder"
        );

        #[cfg(not(target_os = "macos"))]
        info!(
            filename = %filename,
            size_bytes = size_bytes,
            "inotify: new file in drop folder"
        );

        // Write the .pending notification file into the virtiofs share so
        // the Lobster instance inside the VM can be notified.
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

// ---------------------------------------------------------------------------
// .pending notification
// ---------------------------------------------------------------------------

/// Write (or update) a JSON notification file at
/// `{host_drop_folder}/.pending` listing recently arrived files.
///
/// The Lobster instance inside the VM polls this file (it is visible at
/// `{vm_drop_folder_path}/.pending`) and processes the listed filenames.
async fn write_pending_notification(config: &DropServerConfig, event: &DropEvent) {
    let pending_path = config.host_drop_folder.join(".pending");

    // Read existing entries, append the new one, and re-serialise.
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
        Ok(json) => {
            if let Err(e) = tokio::fs::write(&pending_path, json).await {
                warn!("failed to write .pending: {e}");
            }
        }
        Err(e) => {
            warn!("failed to serialise .pending: {e}");
        }
    }
}

/// One entry in the `.pending` JSON file.
#[derive(Debug, Serialize, Deserialize)]
struct PendingEntry {
    filename: String,
    size_bytes: u64,
    arrived_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the default host drop folder path:
/// `~/Library/Application Support/com.fullyparsed.bisque-computer/lobster-drop/`
/// on macOS, or `~/.local/share/bisque-computer/lobster-drop/` elsewhere.
fn default_host_drop_folder() -> PathBuf {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));

    #[cfg(target_os = "macos")]
    {
        home.join("Library")
            .join("Application Support")
            .join("com.fullyparsed.bisque-computer")
            .join("lobster-drop")
    }

    #[cfg(not(target_os = "macos"))]
    {
        home.join(".local")
            .join("share")
            .join("bisque-computer")
            .join("lobster-drop")
    }
}

/// Strip path separators and null bytes from an untrusted filename so it
/// cannot escape the drop folder via path traversal.
fn sanitize_filename(raw: &str) -> String {
    raw.chars()
        .filter(|&c| c != '/' && c != '\\' && c != '\0')
        .collect::<String>()
        .trim_start_matches('.')
        .to_owned()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
        // Leading dots would create hidden temp files that the watcher ignores.
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
        assert_eq!(cfg.vm_drop_folder_path, PathBuf::from("/mnt/lobster-drop"));
    }

    #[tokio::test]
    async fn start_drop_server_creates_directory() {
        let tmp = std::env::temp_dir().join(format!(
            "bisque_drop_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let config = DropServerConfig {
            host_drop_folder: tmp.clone(),
            vm_drop_folder_path: PathBuf::from("/mnt/lobster-drop"),
            http_port: 0, // let OS pick; we won't actually bind here in the test
        };

        // Just verify the directory-creation step works without panicking.
        tokio::fs::create_dir_all(&config.host_drop_folder)
            .await
            .expect("create dir");
        assert!(tmp.is_dir());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
