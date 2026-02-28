use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::time::uptime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Logging verbosity level derived from CLI flags.
pub enum Verbosity {
    /// Errors only (--quiet)
    Quiet,
    /// INFO+ on console (default)
    Normal,
    /// DEBUG+ on console (--verbose)
    Verbose,
}

/// Default log directory: `~/.cache/whisper-models/logs/`
fn default_log_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".cache").join("whisper-models").join("logs"))
}

/// Initialize dual-layer tracing: colored console + daily-rolling file.
///
/// Returns a `WorkerGuard` that **must** be kept alive for the program's
/// lifetime — dropping it flushes the file writer.
pub fn init(verbosity: Verbosity, log_file_override: Option<&PathBuf>) -> Option<WorkerGuard> {
    let console_filter = match verbosity {
        Verbosity::Quiet => EnvFilter::new("error"),
        Verbosity::Normal => EnvFilter::new("info"),
        Verbosity::Verbose => EnvFilter::new("debug"),
    };

    let console_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_target(false)
        .with_filter(console_filter);

    // Try to set up a file layer; if it fails, run console-only.
    match build_file_writer(log_file_override) {
        Some((non_blocking, guard)) => {
            let file_layer = tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_timer(uptime())
                .with_thread_ids(true)
                .with_target(true)
                .with_filter(EnvFilter::new("trace"));

            tracing_subscriber::registry()
                .with(console_layer)
                .with(file_layer)
                .init();

            Some(guard)
        }
        None => {
            tracing_subscriber::registry()
                .with(console_layer)
                .init();

            None
        }
    }
}

/// Create the non-blocking file writer. Returns `None` if the log directory
/// cannot be created (e.g. read-only filesystem).
fn build_file_writer(
    override_path: Option<&PathBuf>,
) -> Option<(tracing_appender::non_blocking::NonBlocking, WorkerGuard)> {
    let log_dir = if let Some(p) = override_path {
        p.parent()
            .map(|d| d.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        default_log_dir()?
    };

    std::fs::create_dir_all(&log_dir).ok()?;

    let file_name = if let Some(p) = override_path {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "transcriber.log".to_string())
    } else {
        "transcriber.log".to_string()
    };

    let file_appender = tracing_appender::rolling::daily(&log_dir, &file_name);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    Some((non_blocking, guard))
}
