#![allow(dead_code)]

use thiserror::Error;

// ── Audio errors ─────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Cannot open audio file: {path}")]
    FileOpen {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Unsupported audio format")]
    UnsupportedFormat,

    #[error("No audio track found in file")]
    NoTrack,

    #[error("Unsupported audio codec")]
    UnsupportedCodec,

    #[error("Audio decode error: {0}")]
    DecodeError(String),

    #[error("Audio file contains no samples")]
    EmptyAudio,

    #[error("Audio too short ({seconds:.1}s) — minimum is 0.5s")]
    TooShort { seconds: f64 },

    #[error("Audio too long ({hours:.1}h) — maximum is 4 hours")]
    TooLong { hours: f64 },
}

// ── Model errors ─────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("Cannot determine home/cache directory")]
    NoCacheDir,

    #[error("Insufficient disk space for model download")]
    InsufficientDiskSpace,

    #[error("Failed to download model after {attempts} attempts: {reason}")]
    DownloadFailed { attempts: u32, reason: String },

    #[error("HTTP error {status} downloading model from {url}")]
    HttpError { status: u16, url: String },

    #[error("Download timed out after {seconds}s")]
    Timeout { seconds: u64 },

    #[error("Model file too small ({size} bytes) — expected at least {expected} bytes for {model} model")]
    FileTooSmall {
        size: u64,
        expected: u64,
        model: String,
    },

    #[error("Failed to load Whisper model: {0}")]
    LoadFailed(String),

    #[error("Model path is not valid UTF-8: {0}")]
    InvalidPath(String),

    #[error("Cannot create cache directory: {path}")]
    CacheDirCreation {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Cannot rename temp file to final path: {0}")]
    RenameFailed(String),
}

// ── Transcription errors ─────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum TranscriptionError {
    #[error("Failed to create Whisper state")]
    StateCreation,

    #[error("Inference failed during transcription")]
    InferenceFailed,

    #[error("Failed to read transcription segments")]
    SegmentRead,

    #[error("Invalid timestamp in segment {index}: start={start}, end={end}")]
    InvalidTimestamp {
        index: i32,
        start: i64,
        end: i64,
    },
}

// ── Output errors ────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum OutputError {
    #[error("Cannot create output file: {path}")]
    FileCreate {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write output: {0}")]
    WriteFailed(String),
}

// ── Exit codes ───────────────────────────────────────────────────────

pub struct ExitCode;

impl ExitCode {
    pub const SUCCESS: i32 = 0;

    // Audio errors (10-12)
    pub const AUDIO_INPUT: i32 = 10;
    pub const AUDIO_DECODE: i32 = 11;
    pub const AUDIO_VALIDATION: i32 = 12;

    // Model errors (20-23)
    pub const MODEL_NOT_FOUND: i32 = 20;
    pub const MODEL_DOWNLOAD: i32 = 21;
    pub const MODEL_INTEGRITY: i32 = 22;
    pub const MODEL_LOAD: i32 = 23;

    // Transcription errors (30)
    pub const TRANSCRIPTION: i32 = 30;

    // Output errors (40)
    pub const OUTPUT_WRITE: i32 = 40;

    // Unknown (99)
    pub const UNKNOWN: i32 = 99;

    /// Walk the anyhow error chain and return the appropriate exit code.
    pub fn from_error(err: &anyhow::Error) -> i32 {
        for cause in err.chain() {
            if let Some(e) = cause.downcast_ref::<AudioError>() {
                return match e {
                    AudioError::FileOpen { .. } | AudioError::UnsupportedFormat => {
                        Self::AUDIO_INPUT
                    }
                    AudioError::NoTrack
                    | AudioError::UnsupportedCodec
                    | AudioError::DecodeError(_) => Self::AUDIO_DECODE,
                    AudioError::EmptyAudio | AudioError::TooShort { .. } | AudioError::TooLong { .. } => {
                        Self::AUDIO_VALIDATION
                    }
                };
            }
            if let Some(e) = cause.downcast_ref::<ModelError>() {
                return match e {
                    ModelError::NoCacheDir | ModelError::CacheDirCreation { .. } => {
                        Self::MODEL_NOT_FOUND
                    }
                    ModelError::InsufficientDiskSpace
                    | ModelError::DownloadFailed { .. }
                    | ModelError::HttpError { .. }
                    | ModelError::Timeout { .. } => Self::MODEL_DOWNLOAD,
                    ModelError::FileTooSmall { .. } => Self::MODEL_INTEGRITY,
                    ModelError::LoadFailed(_)
                    | ModelError::InvalidPath(_)
                    | ModelError::RenameFailed(_) => Self::MODEL_LOAD,
                };
            }
            if cause.downcast_ref::<TranscriptionError>().is_some() {
                return Self::TRANSCRIPTION;
            }
            if cause.downcast_ref::<OutputError>().is_some() {
                return Self::OUTPUT_WRITE;
            }
        }
        Self::UNKNOWN
    }
}
