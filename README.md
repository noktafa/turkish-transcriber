# Turkish Audio Transcriber

Offline Turkish speech-to-text transcriber powered by [whisper.cpp](https://github.com/ggerganov/whisper.cpp) via [whisper-rs](https://github.com/tazz4843/whisper-rs). Native Rust binary with zero Python dependencies.

Accepts MP3, WAV, M4A, OGG, FLAC audio files. Outputs a UTF-8 text file with full transcript and timestamped segments.

## Features

- **Fully offline** — no API keys, no internet after first model download (or use the bundled release)
- **Turkish language optimized** — language forced to `tr` for best accuracy
- **Timestamped output** — each segment includes `[MM:SS -> MM:SS]` timestamps
- **Beam search** — beam size 5 for accurate decoding
- **File picker GUI** — double-click the exe to open a native file selection dialog
- **CLI support** — pass file path and options from the command line
- **Fast** — native compiled binary, multi-threaded inference, no interpreter overhead
- **Small binary** — single executable, no runtime dependencies
- **Optional GPU** — build with `--features cuda` or `--features metal` for GPU acceleration

## Usage

### Double-click
Run `transcriber.exe` — a file picker opens. Select your audio file. The transcript is saved as `<filename>_transcript.txt` next to the original file.

### Command line
```
transcriber recording.mp3
transcriber recording.mp3 --model large-v3
transcriber recording.mp3 --output result.txt
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--model`, `-m` | `medium` | Whisper model size (see table below) |
| `--output`, `-o` | `<input>_transcript.txt` | Output file path |
| `--verbose` | off | Show debug-level output on console |
| `--quiet` | off | Suppress all output except errors |
| `--log-file` | `~/.cache/whisper-models/logs/transcriber.log` | Custom log file path |

### Model sizes

| Model | Download | Speed | Accuracy |
|-------|----------|-------|----------|
| `tiny` | ~75 MB | Fastest | Basic |
| `base` | ~150 MB | Fast | OK |
| `small` | ~500 MB | Moderate | Good |
| **`medium`** | **~1.5 GB** | **Default** | **Recommended** |
| `large-v3` | ~3 GB | Slowest | Best |

Models use the GGML format from [ggerganov/whisper.cpp](https://huggingface.co/ggerganov/whisper.cpp) on HuggingFace.

## Output format

```
=== TRANSCRIPT (Turkish) ===
Source: recording.mp3
Model: whisper-medium
Duration: 176.0s
========================================

Full transcript text here...

=== TIMESTAMPED ===

[00:00 -> 00:07]  First segment text...
[00:07 -> 00:12]  Second segment text...
```

## Building from source

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (1.70+)
- C/C++ compiler (MSVC on Windows, gcc/clang on Linux/Mac)
- CMake (for building whisper.cpp)

### Build

```bash
cargo build --release
```

Output: `target/release/transcriber` (or `transcriber.exe` on Windows)

### GPU acceleration (optional)

```bash
# NVIDIA CUDA
cargo build --release --features cuda

# Apple Metal
cargo build --release --features metal
```

### Bundled release

Place the GGML model file in a `model/` directory next to the executable:
```
transcriber.exe
model/
  ggml-medium.bin
```

The transcriber will use the bundled model instead of downloading.

## Supported audio formats

| Format | Extension | Engine |
|--------|-----------|--------|
| MP3 | `.mp3` | symphonia |
| WAV | `.wav` | symphonia |
| FLAC | `.flac` | symphonia |
| OGG/Vorbis | `.ogg` | symphonia |
| AAC/M4A | `.m4a` | symphonia |

All formats are decoded natively in Rust — no ffmpeg required.

## Logging

A detailed log file is written to `~/.cache/whisper-models/logs/transcriber.log` (daily rolling) with full trace-level detail including timestamps and thread IDs. This is useful for post-mortem debugging.

Use `--verbose` for debug output on the console, or `--quiet` to suppress everything except errors.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 10 | Audio input error (file not found, unsupported format) |
| 11 | Audio decode error (bad codec, corrupt file) |
| 12 | Audio validation error (empty, too short, too long) |
| 20 | Model not found (no cache dir) |
| 21 | Model download failed (network error, timeout, HTTP error) |
| 22 | Model integrity error (file too small / corrupt) |
| 23 | Model load error (whisper.cpp failure) |
| 30 | Transcription error |
| 40 | Output write error |
| 99 | Unknown error |

These exit codes allow CI/CD pipelines and scripts to classify failures programmatically.

## Performance vs Python version

| Metric | Python (faster-whisper) | Rust (whisper.cpp) |
|--------|------------------------|-------------------|
| Startup time | ~3-5s (interpreter + imports) | ~0.1s |
| Binary size | ~200 MB (PyInstaller bundle) | ~5-10 MB |
| Memory overhead | ~100-200 MB (Python runtime) | Minimal |
| Transcription speed | Fast (CTranslate2 int8) | Fast (whisper.cpp) |
| Audio decoding | ffmpeg (external) | symphonia (native Rust) |

## License

MIT
