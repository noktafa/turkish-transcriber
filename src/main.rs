mod audio;
mod errors;
mod logging;
mod model;
mod transcribe;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{debug, error, info};

use errors::ExitCode;
use logging::Verbosity;

/// Transcribe Turkish audio to text using Whisper.
#[derive(Parser)]
#[command(name = "transcriber", version, about)]
struct Cli {
    /// Path to audio file (opens file picker if omitted)
    file: Option<PathBuf>,

    /// Whisper model size
    #[arg(
        short,
        long,
        default_value = "medium",
        value_parser = ["tiny", "base", "small", "medium", "large-v3"]
    )]
    model: String,

    /// Output text file path
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Enable verbose (debug) console output
    #[arg(long)]
    verbose: bool,

    /// Suppress all console output except errors
    #[arg(long, conflicts_with = "verbose")]
    quiet: bool,

    /// Custom log file path (default: ~/.cache/whisper-models/logs/transcriber.log)
    #[arg(long)]
    log_file: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    let verbosity = if cli.quiet {
        Verbosity::Quiet
    } else if cli.verbose {
        Verbosity::Verbose
    } else {
        Verbosity::Normal
    };

    // _guard must live until program exit to flush the log file
    let _guard = logging::init(verbosity, cli.log_file.as_ref());

    log_system_info();

    if let Err(err) = run_app(cli) {
        let code = ExitCode::from_error(&err);

        // Log full error chain to file for post-mortem
        error!("Fatal error (exit code {code}): {err:#}");

        // User-friendly message to console (tracing handles this via the
        // error! macro above, but also print the top-level for clarity)
        eprintln!("Error: {err}");

        // If launched with no args (double-click), wait before closing
        if std::env::args().len() == 1 {
            eprintln!();
            eprintln!("Press Enter to exit...");
            let _ = std::io::stdin().read_line(&mut String::new());
        }

        std::process::exit(code);
    }
}

fn run_app(cli: Cli) -> Result<()> {
    let audio_path = match cli.file {
        Some(p) => p,
        None => match pick_file_gui() {
            Some(p) => p,
            None => {
                info!("No file selected.");
                return Ok(());
            }
        },
    };

    let audio_path = std::fs::canonicalize(&audio_path)
        .with_context(|| format!("File not found: {}", audio_path.display()))?;

    if !audio_path.is_file() {
        anyhow::bail!("Not a file: {}", audio_path.display());
    }

    let output_path = cli.output.unwrap_or_else(|| {
        let stem = audio_path.file_stem().unwrap_or_default();
        let parent = audio_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        parent.join(format!("{}_transcript.txt", stem.to_string_lossy()))
    });

    transcribe::run(&audio_path, &cli.model, &output_path)?;

    // If launched with no args (double-click), wait before closing the console
    if std::env::args().len() == 1 {
        println!();
        println!("Press Enter to exit...");
        let _ = std::io::stdin().read_line(&mut String::new());
    }

    Ok(())
}

/// Log system info at startup for diagnostics.
fn log_system_info() {
    debug!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
        "System info"
    );
}

/// Open a native file-picker dialog.
fn pick_file_gui() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Select an audio file to transcribe")
        .add_filter("MP3 files", &["mp3"])
        .add_filter(
            "Audio files",
            &["mp3", "wav", "m4a", "ogg", "flac", "wma"],
        )
        .add_filter("All files", &["*"])
        .pick_file()
}
