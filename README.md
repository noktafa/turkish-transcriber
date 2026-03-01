<p align="center">
  <img src="logo.svg" alt="Turkish Transcriber" width="200">
</p>

<h1 align="center">Turkish Audio Transcriber</h1>

<p align="center">
  Offline Turkish speech-to-text transcriber powered by <a href="https://github.com/ggerganov/whisper.cpp">whisper.cpp</a>.<br>
  Single executable. No internet required. No Python. No ffmpeg.
</p>

<p align="center">
  <a href="https://github.com/noktafa/turkish-transcriber/releases/latest"><strong>Download latest release</strong></a>
</p>

---

## Quick Start

1. **Download** the latest `.zip` from [Releases](https://github.com/noktafa/turkish-transcriber/releases/latest)
2. **Extract** the zip anywhere on your computer
3. **Double-click** `turkish-transcriber.exe` — a file picker opens
4. **Select** your audio file (MP3, WAV, M4A, OGG, FLAC)
5. **Done** — the transcript is saved as `<filename>_transcript.txt` next to your audio file

That's it. No installation, no setup, no dependencies.

> **First run:** If you don't have a bundled model, the transcriber will automatically download the Whisper medium model (~1.5 GB) on first use. After that, everything works offline.

## Command Line

```
turkish-transcriber recording.mp3
turkish-transcriber recording.mp3 --model large-v3
turkish-transcriber recording.mp3 --output result.txt
turkish-transcriber recording.mp3 --verbose
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `--model`, `-m` | `medium` | Whisper model size (see table below) |
| `--output`, `-o` | `<input>_transcript.txt` | Output file path |
| `--verbose` | off | Show detailed debug output on console |
| `--quiet` | off | Suppress all output except errors |
| `--log-file` | auto | Custom log file path |

## Features

- **Fully offline** — no API keys, no internet after first model download
- **Turkish optimized** — language forced to `tr` for best accuracy
- **Timestamped output** — each segment includes `[MM:SS -> MM:SS]` timestamps
- **Beam search decoding** — beam size 5 for accurate results
- **File picker GUI** — double-click to open a native file selection dialog
- **Multi-threaded** — uses all available CPU cores automatically
- **Structured logging** — detailed log file for debugging at `~/.cache/whisper-models/logs/`
- **Retry logic** — model downloads retry 3 times with exponential backoff
- **Typed exit codes** — every failure has a specific exit code for scripting

## Model Sizes

| Model | Size | Speed | Accuracy | Use case |
|-------|------|-------|----------|----------|
| `tiny` | ~75 MB | Fastest | Basic | Quick drafts, testing |
| `base` | ~150 MB | Fast | OK | Short clips |
| `small` | ~500 MB | Moderate | Good | General use |
| **`medium`** | **~1.5 GB** | **Balanced** | **Recommended** | **Best quality/speed tradeoff** |
| `large-v3` | ~3 GB | Slowest | Best | Maximum accuracy |

Models are downloaded automatically from [HuggingFace](https://huggingface.co/ggerganov/whisper.cpp) on first use and cached locally.

## Output Format

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

## Supported Audio Formats

| Format | Extensions |
|--------|-----------|
| MP3 | `.mp3` |
| WAV | `.wav` |
| FLAC | `.flac` |
| OGG/Vorbis | `.ogg` |
| AAC/M4A | `.m4a` |

All formats are decoded natively — no ffmpeg required.

## Exit Codes

For scripting and CI/CD integration:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 10 | Audio input error (file not found, unsupported format) |
| 11 | Audio decode error (bad codec, corrupt file) |
| 12 | Audio validation error (empty, too short, too long) |
| 20 | Model not found |
| 21 | Model download failed (network, timeout) |
| 22 | Model integrity error (corrupt download) |
| 23 | Model load error |
| 30 | Transcription error |
| 40 | Output write error |
| 99 | Unknown error |

## Building from Source

### Prerequisites

- [Rust](https://rustup.rs/) 1.70+
- C/C++ compiler (MSVC on Windows, gcc/clang on Linux/Mac)
- CMake 3.15+

### Build

```bash
cargo build --release
```

The binary will be at `target/release/turkish-transcriber` (`.exe` on Windows).

### GPU Acceleration (optional)

```bash
# NVIDIA CUDA
cargo build --release --features cuda

# Apple Metal
cargo build --release --features metal
```

### Bundled Model

To distribute without requiring a download, place the model next to the executable:

```
turkish-transcriber.exe
model/
  ggml-medium.bin
```

## License

MIT

---

<p align="center"><i>

> Ervah-ı ezelde levh-i kalemde<br>
> Benim bahtım kara yazılmış meğer<br>
> Gönül perişandır devr-i alemde<br>
> Bir günüm şadlıksız geçiyor meğer
>
> Gönül gülşeninde har bitti bugün<br>
> Bülbülün figanı göklere erdi<br>
> Felek sillesini vurdu başıma<br>
> Beni bir onulmaz derde düşürdü
>
> Sümmani'yim ey dil gönül perişan<br>
> Gözlerim yollarda kalmışım hayran<br>
> Aradım derdime bulmadım derman<br>
> Tabipler yaramı azdırır meğer
>
> — **Aşık Sümmani**

</i></p>

<p align="center">
<sub>
Bülbülün figanı göklere erişir ama yazıya dökülmezse kaybolur.
Bu araç, Türkçe sözün — ister bir ozanın türküsü, ister bir ananın ninnisi olsun — sessizliğe gömülmemesi için yazıldı.
Her ses bir iz bırakmalı.
</sub>
</p>
