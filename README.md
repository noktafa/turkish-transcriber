# Turkish Audio Transcriber

Offline Turkish speech-to-text transcriber powered by [OpenAI Whisper](https://github.com/openai/whisper) via [faster-whisper](https://github.com/SYSTRAN/faster-whisper) (CTranslate2, CPU int8).

Accepts MP3, WAV, M4A, OGG, FLAC, and WMA audio files. Outputs a UTF-8 text file with full transcript and timestamped segments.

## Features

- **Fully offline** — no API keys, no internet after first model download (or use the bundled release)
- **Turkish language optimized** — language forced to `tr` for best accuracy
- **Timestamped output** — each segment includes `[MM:SS -> MM:SS]` timestamps
- **VAD filtering** — Voice Activity Detection skips silence automatically
- **File picker GUI** — double-click the exe to open a file selection dialog
- **CLI support** — pass file path and options from the command line
- **Portable release** — bundled with VC++ runtime and Whisper model, zero dependencies on target machine

## Usage

### Double-click
Run `transcriber.exe` — a file picker opens. Select your audio file. The transcript is saved as `<filename>_transcript.txt` next to the original file.

### Command line
```
transcriber.exe recording.mp3
transcriber.exe recording.mp3 --model large-v3
transcriber.exe recording.mp3 --output result.txt
```

### Python
```
python transcriber.py recording.mp3
python transcriber.py recording.mp3 -m small -o output.txt
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--model`, `-m` | `medium` | Whisper model size (see table below) |
| `--output`, `-o` | `<input>_transcript.txt` | Output file path |

### Model sizes

| Model | Download | Speed | Accuracy |
|-------|----------|-------|----------|
| `tiny` | ~75 MB | Fastest | Basic |
| `base` | ~150 MB | Fast | OK |
| `small` | ~500 MB | Moderate | Good |
| **`medium`** | **~1.5 GB** | **Default** | **Recommended** |
| `large-v3` | ~3 GB | Slowest | Best |

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

## Installation (from source)

```bash
pip install faster-whisper
python transcriber.py your_audio.mp3
```

The Whisper model downloads automatically on first run and is cached at `~/.cache/whisper-models/`.

## Building the executable

```bash
pip install faster-whisper pyinstaller
python build_exe.py
```

Output: `dist/transcriber.exe`

## Requirements

- Python 3.10+ (for running from source)
- No requirements for the bundled release (Windows x64)

## License

MIT
