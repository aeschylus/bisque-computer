//! Dev logging module for bisque-computer.
//!
//! Enabled by setting the `BISQUE_LOG` environment variable to any non-empty value
//! before launching the app:
//!
//! ```sh
//! BISQUE_LOG=1 bisque-computer
//! ```
//!
//! When active, all `tracing` events (info, warn, error, debug, trace) are written
//! to `~/bisque-computer.log` with RFC 3339 timestamps and log levels. A custom
//! panic hook is also installed so that panics are recorded to the log file before
//! the default panic handler runs.
//!
//! When `BISQUE_LOG` is not set the function is a no-op and returns `None`, leaving
//! stdout/stderr behaviour identical to the unlogged build.

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::prelude::*;

/// Initialise file-based logging if `BISQUE_LOG` is set.
///
/// Returns an `Option<WorkerGuard>` that **must be kept alive** for the
/// duration of the process. Dropping it flushes and closes the log file.
/// Store the returned guard in a local binding in `main()`.
///
/// # Behaviour
///
/// - Log file: `~/bisque-computer.log` (created or appended on each run).
/// - Format: `YYYY-MM-DDTHH:MM:SSZ  LEVEL  target: message`
/// - Panic hook: captures the panic location and message, writes them as an
///   ERROR event, then calls the previous panic handler so the process still
///   aborts with a backtrace.
///
/// # Example
///
/// ```rust
/// let _log_guard = logging::init();
/// ```
pub fn init() -> Option<WorkerGuard> {
    // Only activate when the user explicitly opts in.
    if std::env::var("BISQUE_LOG").unwrap_or_default().is_empty() {
        return None;
    }

    let log_path = log_file_path();

    // Ensure the parent directory exists (it's always ~/, which exists, but
    // be defensive for unusual $HOME configurations).
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Open the log file for appending (create it if absent).
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("bisque-computer: failed to open log file");

    // Wrap in a non-blocking writer. The returned guard must be kept alive.
    let (non_blocking, guard) = tracing_appender::non_blocking(log_file);

    // Build a subscriber that writes timestamped plain-text lines.
    // SystemTime is always available (no extra feature flags required).
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false) // no colour codes in the file
            .with_timer(tracing_subscriber::fmt::time::SystemTime)
            .with_target(true)
            .with_thread_ids(false)
            .with_file(false),
    );

    // Install as the global default. Panics if a subscriber is already set,
    // which should not happen in normal usage.
    tracing::subscriber::set_global_default(subscriber)
        .expect("bisque-computer: failed to set global tracing subscriber");

    // Install the panic hook *after* the subscriber is live so tracing::error!
    // will actually reach the file writer.
    install_panic_hook();

    let display_path = log_path.display().to_string();
    tracing::info!("bisque-computer logging initialised — writing to {}", display_path);
    eprintln!("[bisque] logging to {}", display_path);

    Some(guard)
}

/// Return the absolute path for the log file: `~/bisque-computer.log`.
fn log_file_path() -> PathBuf {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"));
    home.join("bisque-computer.log")
}

/// Install a panic hook that logs the panic as a `tracing::error!` event
/// before delegating to the previously-installed handler (usually the default
/// Rust panic handler that prints a backtrace and aborts).
///
/// This is a pure function in the sense that it only captures the previous
/// hook via `std::panic::take_hook` and composes a new closure around it —
/// the side-effectful registration is isolated here at the boundary.
fn install_panic_hook() {
    let prev_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        // Extract location (file:line) if available.
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown location>".to_string());

        // Extract the panic message (works for both `panic!("msg")` and
        // `panic!("{}", expr)` since PanicInfo::payload() is `dyn Any`).
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        };

        tracing::error!(
            location = %location,
            "PANIC: {}",
            message
        );

        // Delegate to the previous handler so the process still aborts with
        // the standard Rust panic output (backtrace, etc.).
        prev_hook(info);
    }));
}
