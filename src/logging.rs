//! Logging initialisation for bisque-computer.
//!
//! When the `BISQUE_LOG` environment variable is set to `1`, structured
//! logs are written to the OS log directory under `bisque-computer/bisque.log`.
//! Otherwise only stderr output (filtered by `RUST_LOG`) is enabled.
//!
//! Returns a guard that must be kept alive for the duration of the process
//! so that buffered log lines are flushed on exit.

use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub struct LogGuard {
    _file_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

/// Initialise the global tracing subscriber.
///
/// Call once from `main`, store the returned `LogGuard` in a local variable
/// for the duration of the process.
pub fn init() -> LogGuard {
    let file_guard = if std::env::var("BISQUE_LOG").as_deref() == Ok("1") {
        let dir = log_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        let _ = std::fs::create_dir_all(&dir);
        let file_appender = tracing_appender::rolling::never(dir, "bisque.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false);

        tracing_subscriber::registry()
            .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
            .with(fmt::layer().with_writer(std::io::stderr))
            .with(file_layer)
            .init();

        Some(guard)
    } else {
        tracing_subscriber::registry()
            .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
            .with(fmt::layer().with_writer(std::io::stderr))
            .init();

        None
    };

    LogGuard { _file_guard: file_guard }
}

fn log_dir() -> Option<std::path::PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        let mut p = std::path::PathBuf::from(xdg);
        p.push("bisque-computer");
        return Some(p);
    }
    let home = std::env::var("HOME").ok()?;
    let mut p = std::path::PathBuf::from(home);
    #[cfg(target_os = "macos")]
    {
        p.push("Library");
        p.push("Logs");
    }
    #[cfg(not(target_os = "macos"))]
    {
        p.push(".local");
        p.push("share");
    }
    p.push("bisque-computer");
    Some(p)
}
