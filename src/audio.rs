use std::path::Path;

use anyhow::Result;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tracing::{debug, trace, warn};

use crate::errors::AudioError;

const WHISPER_SAMPLE_RATE: u32 = 16_000;
const MIN_AUDIO_SECONDS: f64 = 0.5;
const MAX_AUDIO_HOURS: f64 = 4.0;

/// Load an audio file, decode to f32 mono, and resample to 16 kHz.
#[tracing::instrument(skip_all, fields(path = %path.display()))]
pub fn load_audio(path: &Path) -> Result<Vec<f32>> {
    // Log file metadata
    if let Ok(meta) = std::fs::metadata(path) {
        debug!(size_bytes = meta.len(), "Audio file metadata");
    }

    let file = std::fs::File::open(path).map_err(|e| AudioError::FileOpen {
        path: path.display().to_string(),
        source: e,
    })?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|_| AudioError::UnsupportedFormat)?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(AudioError::NoTrack)?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

    debug!(sample_rate, channels, "Detected audio format");

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|_| AudioError::UnsupportedCodec)?;

    let mut pcm: Vec<f32> = Vec::new();
    let mut packet_count: u64 = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                return Err(AudioError::DecodeError(e.to_string()).into());
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder
            .decode(&packet)
            .map_err(|e| AudioError::DecodeError(e.to_string()))?;
        let spec = *decoded.spec();
        let frames = decoded.frames();
        let ch = spec.channels.count();

        if frames == 0 {
            continue;
        }

        let mut sbuf = SampleBuffer::<f32>::new(frames as u64, spec);
        sbuf.copy_interleaved_ref(decoded);

        // Downmix interleaved multi-channel to mono
        for chunk in sbuf.samples().chunks(ch) {
            pcm.push(chunk.iter().sum::<f32>() / ch as f32);
        }

        packet_count += 1;
        if packet_count % 500 == 0 {
            trace!(packets = packet_count, samples = pcm.len(), "Decoding progress");
        }
    }

    debug!(total_packets = packet_count, total_samples = pcm.len(), "Decode complete");

    // Resample to 16 kHz if the source rate differs
    if sample_rate != WHISPER_SAMPLE_RATE {
        debug!(from = sample_rate, to = WHISPER_SAMPLE_RATE, "Resampling");
        pcm = resample(&pcm, sample_rate, WHISPER_SAMPLE_RATE);
    }

    // ── Post-decode validation ───────────────────────────────────────
    if pcm.is_empty() {
        return Err(AudioError::EmptyAudio.into());
    }

    let duration_secs = pcm.len() as f64 / WHISPER_SAMPLE_RATE as f64;

    if duration_secs < MIN_AUDIO_SECONDS {
        return Err(AudioError::TooShort {
            seconds: duration_secs,
        }
        .into());
    }

    let duration_hours = duration_secs / 3600.0;
    if duration_hours > MAX_AUDIO_HOURS {
        return Err(AudioError::TooLong {
            hours: duration_hours,
        }
        .into());
    }

    if duration_secs < 1.0 {
        warn!(duration_secs, "Audio is very short — results may be poor");
    }

    debug!(duration_secs = format!("{duration_secs:.1}"), samples = pcm.len(), "Audio loaded");

    Ok(pcm)
}

/// Linear-interpolation resampler (adequate for speech recognition).
#[tracing::instrument(skip_all, fields(from_rate, to_rate))]
fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if input.is_empty() || from_rate == to_rate {
        return input.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (input.len() as f64 / ratio).ceil() as usize;
    let mut output = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src = i as f64 * ratio;
        let idx = src as usize;
        let frac = (src - idx as f64) as f32;

        let sample = if idx + 1 < input.len() {
            input[idx] * (1.0 - frac) + input[idx + 1] * frac
        } else {
            input[idx.min(input.len() - 1)]
        };
        output.push(sample);
    }

    debug!(input_samples = input.len(), output_samples = output.len(), "Resample complete");

    output
}
