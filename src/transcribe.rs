use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::{debug, info, info_span, warn};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::audio;
use crate::errors::{ModelError, OutputError, TranscriptionError};
use crate::model;

/// A single transcribed segment with timestamps (in seconds).
struct Segment {
    start: f64,
    end: f64,
    pub(crate) text: String,
}

/// Run the full transcription pipeline and write the output file.
#[tracing::instrument(skip_all, fields(
    audio = %audio_path.display(),
    model = model_size,
    output = %output_path.display(),
))]
pub fn run(audio_path: &Path, model_size: &str, output_path: &Path) -> Result<()> {
    let pipeline_start = Instant::now();

    // ── Resolve model ────────────────────────────────────────────────
    let (model_path, is_bundled) = {
        let _span = info_span!("resolve_model").entered();
        model::resolve_model(model_size)?
    };
    let label = if is_bundled { "bundled" } else { "cached/downloaded" };
    info!(
        model = %model_path.file_name().unwrap_or_default().to_string_lossy(),
        source = label,
        "Model resolved"
    );
    info!(
        input = %audio_path.file_name().unwrap_or_default().to_string_lossy(),
        "Input file"
    );
    eprintln!(
        "[1/5] Model: whisper-{model_size} ({label})"
    );

    // ── Load audio ───────────────────────────────────────────────────
    let samples = {
        let _span = info_span!("load_audio").entered();
        let file_name = audio_path.file_name().unwrap_or_default().to_string_lossy();
        eprintln!("[2/5] Decoding audio: {file_name}");
        let t0 = Instant::now();
        let s = audio::load_audio(audio_path)?;
        let secs = t0.elapsed().as_secs_f64();
        info!(elapsed_secs = format!("{secs:.1}"), "Audio loaded");
        eprintln!("       Decoded in {secs:.1}s");
        s
    };

    let audio_duration_secs = samples.len() as f64 / 16_000.0;
    let audio_mins = audio_duration_secs / 60.0;
    eprintln!("       Audio length: {audio_mins:.1} minutes");

    // ── Load Whisper model ───────────────────────────────────────────
    let ctx = {
        let _span = info_span!("load_whisper").entered();
        eprintln!("[3/5] Loading whisper-{model_size} model...");
        let t0 = Instant::now();
        let model_str = model_path
            .to_str()
            .ok_or_else(|| ModelError::InvalidPath(model_path.display().to_string()))?;
        // GPU is auto-enabled when compiled with vulkan/cuda feature
        let c = WhisperContext::new_with_params(model_str, WhisperContextParameters::default())
            .map_err(|e| ModelError::LoadFailed(e.to_string()))?;
        let secs = t0.elapsed().as_secs_f64();
        info!(elapsed_secs = format!("{secs:.1}"), "Whisper model loaded");
        eprintln!("       Model loaded in {secs:.1}s");
        c
    };

    // ── Transcribe ───────────────────────────────────────────────────
    let (mut segments, transcribe_secs) = {
        let _span = info_span!("transcribe").entered();
        info!("Transcribing...");
        eprintln!("[4/5] Transcribing ({audio_mins:.1} min of audio)...");

        let pb = ProgressBar::new(100);
        pb.set_style(
            ProgressStyle::with_template(
                "       [{bar:40.green/dim}] {pos}% | elapsed: {elapsed_precise} | ETA: {eta}",
            )
            .unwrap()
            .progress_chars("=> "),
        );
        pb.set_position(0);

        let t0 = Instant::now();

        let mut state = ctx
            .create_state()
            .map_err(|e| TranscriptionError::StateCreation(e.to_string()))?;

        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });
        params.set_language(Some("tr"));
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_no_speech_thold(0.6);

        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);
        params.set_n_threads(threads);
        debug!(threads, "Inference threads");

        // Progress callback — drives the progress bar
        let pb_cb = pb.clone();
        params.set_progress_callback_safe(move |progress: i32| {
            pb_cb.set_position(progress.max(0) as u64);
        });

        // Segment callback — show live segments as they arrive
        let seg_count = Arc::new(Mutex::new(0u32));
        let seg_count_cb = Arc::clone(&seg_count);
        let pb_seg = pb.clone();
        params.set_segment_callback_safe_lossy(move |data: whisper_rs::SegmentCallbackData| {
            let mut count = seg_count_cb.lock().unwrap();
            *count += 1;
            let text = data.text.trim();
            if !text.is_empty() {
                let preview: String = text.chars().take(60).collect();
                pb_seg.set_message(format!("[seg {count}] {preview}"));
            }
        });

        state
            .full(params, &samples)
            .map_err(|e| TranscriptionError::InferenceFailed(e.to_string()))?;

        pb.finish_and_clear();
        let elapsed = t0.elapsed().as_secs_f64();

        // ── Collect segments ─────────────────────────────────────────
        let n = state.full_n_segments();

        let mut segments: Vec<Segment> = Vec::with_capacity(n as usize);
        let mut skipped = 0u32;
        let mut total_chars: usize = 0;

        for i in 0..n {
            let seg = match state.get_segment(i) {
                Some(s) => s,
                None => {
                    skipped += 1;
                    continue;
                }
            };

            let t0 = seg.start_timestamp();
            let t1 = seg.end_timestamp();

            // Validate timestamps
            if t0 < 0 || t1 < 0 {
                warn!(segment = i, start = t0, end = t1, "Negative timestamp — skipping segment");
                skipped += 1;
                continue;
            }
            if t1 < t0 {
                warn!(segment = i, start = t0, end = t1, "Inverted timestamps — skipping segment");
                skipped += 1;
                continue;
            }

            let text = match seg.to_str_lossy() {
                Ok(t) => t,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            let trimmed = text.trim().to_string();
            if trimmed.is_empty() {
                debug!(segment = i, "Empty text — skipping segment");
                skipped += 1;
                continue;
            }

            total_chars += trimmed.len();
            segments.push(Segment {
                start: t0 as f64 / 100.0,
                end: t1 as f64 / 100.0,
                text: trimmed,
            });
        }

        // ── Performance metrics ──────────────────────────────────────
        let realtime_factor = if audio_duration_secs > 0.0 {
            elapsed / audio_duration_secs
        } else {
            0.0
        };

        info!(
            elapsed_secs = format!("{elapsed:.1}"),
            audio_duration_secs = format!("{audio_duration_secs:.1}"),
            realtime_factor = format!("{realtime_factor:.2}x"),
            segments = segments.len(),
            skipped,
            total_chars,
            "Transcription complete"
        );
        eprintln!(
            "       Done in {elapsed:.1}s ({realtime_factor:.2}x realtime) — {} segments, {} chars",
            segments.len(),
            total_chars,
        );

        (segments, elapsed)
    };

    // ── Post-process Turkish text ────────────────────────────────────
    {
        let _span = info_span!("postprocess").entered();
        for seg in &mut segments {
            seg.text = crate::postprocess::process(&seg.text);
        }
        info!(segments = segments.len(), "Post-processing complete");
    }

    // ── Write output ─────────────────────────────────────────────────
    {
        let _span = info_span!("write_output").entered();
        write_output(output_path, audio_path, model_size, transcribe_secs, &segments)?;
        info!(path = %output_path.display(), "Output written");
    }

    let total_elapsed = pipeline_start.elapsed().as_secs_f64();
    info!(total_secs = format!("{total_elapsed:.1}"), "Pipeline complete");
    eprintln!(
        "[5/5] Saved to: {}",
        output_path.display()
    );
    eprintln!("       Total time: {total_elapsed:.1}s");

    Ok(())
}

/// Write the transcript file matching the Python version's format exactly.
#[tracing::instrument(skip_all, fields(path = %path.display()))]
fn write_output(
    path: &Path,
    source: &Path,
    model_size: &str,
    duration: f64,
    segments: &[Segment],
) -> Result<()> {
    use std::io::Write;

    let mut f = std::fs::File::create(path).map_err(|e| OutputError::FileCreate {
        path: path.display().to_string(),
        source: e,
    })?;

    macro_rules! w {
        ($($arg:tt)*) => {
            write!(f, $($arg)*).map_err(|e| OutputError::WriteFailed(e.to_string()))?
        };
    }

    if segments.is_empty() {
        w!("No speech detected in the audio.\n");
        return Ok(());
    }

    // Header
    w!("=== TRANSCRIPT (Turkish) ===\n");
    w!(
        "Source: {}\n",
        source.file_name().unwrap_or_default().to_string_lossy()
    );
    w!("Model: whisper-{model_size}\n");
    w!("Duration: {duration:.1}s\n");
    w!("{}\n", "=".repeat(40));
    w!("\n");

    // Full text
    let full: String = segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    w!("{full}\n\n");

    // Timestamped segments
    w!("=== TIMESTAMPED ===\n\n");
    for seg in segments {
        let (sm, ss) = (seg.start as u64 / 60, seg.start as u64 % 60);
        let (em, es) = (seg.end as u64 / 60, seg.end as u64 % 60);
        w!("[{sm:02}:{ss:02} -> {em:02}:{es:02}]  {}\n", seg.text);
    }

    Ok(())
}
