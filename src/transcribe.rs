use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use tracing::{debug, info, info_span, warn};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::audio;
use crate::errors::{ModelError, OutputError, TranscriptionError};
use crate::model;

/// A single transcribed segment with timestamps (in seconds).
struct Segment {
    start: f64,
    end: f64,
    text: String,
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

    // ── Load audio ───────────────────────────────────────────────────
    let samples = {
        let _span = info_span!("load_audio").entered();
        let t0 = Instant::now();
        let s = audio::load_audio(audio_path)?;
        info!(elapsed_secs = format!("{:.1}", t0.elapsed().as_secs_f64()), "Audio loaded");
        s
    };

    let audio_duration_secs = samples.len() as f64 / 16_000.0;

    // ── Load Whisper model ───────────────────────────────────────────
    let ctx = {
        let _span = info_span!("load_whisper").entered();
        let t0 = Instant::now();
        let model_str = model_path
            .to_str()
            .ok_or_else(|| ModelError::InvalidPath(model_path.display().to_string()))?;
        let c = WhisperContext::new_with_params(model_str, WhisperContextParameters::default())
            .map_err(|e| ModelError::LoadFailed(e.to_string()))?;
        info!(elapsed_secs = format!("{:.1}", t0.elapsed().as_secs_f64()), "Whisper model loaded");
        c
    };

    // ── Transcribe ───────────────────────────────────────────────────
    let (segments, transcribe_secs) = {
        let _span = info_span!("transcribe").entered();
        info!("Transcribing...");
        let t0 = Instant::now();

        let mut state = ctx
            .create_state()
            .map_err(|_| TranscriptionError::StateCreation)?;

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

        state
            .full(params, &samples)
            .map_err(|_| TranscriptionError::InferenceFailed)?;

        let elapsed = t0.elapsed().as_secs_f64();

        // ── Collect segments ─────────────────────────────────────────
        let n = state
            .full_n_segments()
            .map_err(|_| TranscriptionError::SegmentRead)?;

        let mut segments: Vec<Segment> = Vec::with_capacity(n as usize);
        let mut skipped = 0u32;
        let mut total_chars: usize = 0;

        for i in 0..n {
            let text = state
                .full_get_segment_text(i)
                .map_err(|_| TranscriptionError::SegmentRead)?;
            let t0 = state
                .full_get_segment_t0(i)
                .map_err(|_| TranscriptionError::SegmentRead)?;
            let t1 = state
                .full_get_segment_t1(i)
                .map_err(|_| TranscriptionError::SegmentRead)?;

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

        (segments, elapsed)
    };

    // ── Write output ─────────────────────────────────────────────────
    {
        let _span = info_span!("write_output").entered();
        write_output(output_path, audio_path, model_size, transcribe_secs, &segments)?;
        info!(path = %output_path.display(), "Output written");
    }

    let total_elapsed = pipeline_start.elapsed().as_secs_f64();
    info!(total_secs = format!("{total_elapsed:.1}"), "Pipeline complete");

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
