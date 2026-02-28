use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{debug, info, warn};

use crate::errors::ModelError;

/// Maximum number of download attempts.
const MAX_RETRIES: u32 = 3;

/// Backoff delays in seconds for each retry attempt.
const BACKOFF_SECS: &[u64] = &[1, 2, 4];

/// HTTP connect timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// HTTP total download timeout (10 minutes — large models).
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

/// Minimum expected model file sizes in bytes (approximate).
fn min_model_size(model: &str) -> u64 {
    match model {
        "tiny" => 50_000_000,       // ~75 MB
        "base" => 100_000_000,      // ~150 MB
        "small" => 300_000_000,     // ~500 MB
        "medium" => 1_000_000_000,  // ~1.5 GB
        "large-v3" => 2_000_000_000, // ~3 GB
        _ => 0,
    }
}

/// Check for a bundled model next to the executable, then the cache.
/// Downloads the GGML model from HuggingFace if not found.
#[tracing::instrument(skip_all, fields(model_size = size))]
pub fn resolve_model(size: &str) -> Result<(PathBuf, bool)> {
    // 1. Bundled model next to the binary
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let bundled = exe_dir.join("model").join(model_filename(size));
    if bundled.is_file() {
        info!(path = %bundled.display(), "Using bundled model");
        return Ok((bundled, true));
    }

    // Also check for a generic "model/model.bin" (legacy layout)
    let bundled_legacy = exe_dir.join("model").join("model.bin");
    if bundled_legacy.is_file() {
        info!(path = %bundled_legacy.display(), "Using bundled model (legacy layout)");
        return Ok((bundled_legacy, true));
    }

    debug!("No bundled model found, checking cache");

    // 2. Cached model in ~/.cache/whisper-models/
    let cache_dir = dirs::home_dir()
        .map(|h| h.join(".cache").join("whisper-models"))
        .ok_or(ModelError::NoCacheDir)?;

    std::fs::create_dir_all(&cache_dir).map_err(|e| ModelError::CacheDirCreation {
        path: cache_dir.display().to_string(),
        source: e,
    })?;

    let cached = cache_dir.join(model_filename(size));
    if cached.is_file() {
        info!(path = %cached.display(), "Using cached model");

        // Validate cached file size
        if let Ok(meta) = std::fs::metadata(&cached) {
            let min = min_model_size(size);
            if min > 0 && meta.len() < min {
                warn!(
                    size = meta.len(),
                    expected_min = min,
                    "Cached model file is suspiciously small — re-downloading"
                );
                let _ = std::fs::remove_file(&cached);
            } else {
                return Ok((cached, false));
            }
        } else {
            return Ok((cached, false));
        }
    }

    // 3. Download with retry
    debug!("Model not in cache, downloading");
    download_model_with_retry(size, &cached)?;
    Ok((cached, false))
}

fn model_filename(size: &str) -> String {
    format!("ggml-{size}.bin")
}

/// Download with exponential backoff retry.
fn download_model_with_retry(size: &str, dest: &Path) -> Result<()> {
    let mut last_err = String::new();

    for attempt in 1..=MAX_RETRIES {
        match download_model(size, dest) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = format!("{e:#}");
                warn!(attempt, max = MAX_RETRIES, error = %last_err, "Download attempt failed");

                // Clean up partial file
                let tmp = dest.with_extension("part");
                if tmp.exists() {
                    debug!(path = %tmp.display(), "Cleaning up temp file");
                    let _ = std::fs::remove_file(&tmp);
                }

                if attempt < MAX_RETRIES {
                    let delay = BACKOFF_SECS
                        .get(attempt as usize - 1)
                        .copied()
                        .unwrap_or(4);
                    info!(delay_secs = delay, "Retrying after backoff");
                    std::thread::sleep(Duration::from_secs(delay));
                }
            }
        }
    }

    Err(ModelError::DownloadFailed {
        attempts: MAX_RETRIES,
        reason: last_err,
    }
    .into())
}

#[tracing::instrument(skip_all, fields(model_size = size))]
fn download_model(size: &str, dest: &Path) -> Result<()> {
    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{size}.bin"
    );

    info!(url = %url, "Downloading model");

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(DOWNLOAD_TIMEOUT)
        .build()
        .map_err(|e| ModelError::DownloadFailed {
            attempts: 1,
            reason: format!("Cannot build HTTP client: {e}"),
        })?;

    let resp = client.get(&url).send().map_err(|e| {
        if e.is_timeout() {
            ModelError::Timeout {
                seconds: DOWNLOAD_TIMEOUT.as_secs(),
            }
        } else {
            ModelError::DownloadFailed {
                attempts: 1,
                reason: e.to_string(),
            }
        }
    })?;

    if !resp.status().is_success() {
        return Err(ModelError::HttpError {
            status: resp.status().as_u16(),
            url,
        }
        .into());
    }

    let total = resp.content_length().unwrap_or(0);
    debug!(content_length = total, "Download started");

    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template("{bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("##-"),
    );

    // Stream to a temp file, then rename (atomic-ish)
    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp).map_err(|e| ModelError::CacheDirCreation {
        path: tmp.display().to_string(),
        source: e,
    })?;

    let mut reader = pb.wrap_read(resp);
    std::io::copy(&mut reader, &mut file).map_err(|e| ModelError::DownloadFailed {
        attempts: 1,
        reason: format!("I/O error during download: {e}"),
    })?;
    file.flush().map_err(|e| ModelError::DownloadFailed {
        attempts: 1,
        reason: format!("Flush failed: {e}"),
    })?;

    pb.finish_with_message("Download complete");

    // Validate downloaded file size
    let min = min_model_size(size);
    if min > 0 {
        let actual = std::fs::metadata(&tmp)
            .map(|m| m.len())
            .unwrap_or(0);
        if actual < min {
            let _ = std::fs::remove_file(&tmp);
            return Err(ModelError::FileTooSmall {
                size: actual,
                expected: min,
                model: size.to_string(),
            }
            .into());
        }
    }

    std::fs::rename(&tmp, dest).map_err(|e| ModelError::RenameFailed(e.to_string()))?;
    info!(path = %dest.display(), "Model saved");
    Ok(())
}
