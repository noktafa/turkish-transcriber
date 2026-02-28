"""
Turkish MP3 Audio Transcriber
Transcribes Turkish speech from MP3 files to text using faster-whisper (CTranslate2, CPU int8).
"""

import sys
import os
import io
import time
import argparse
from pathlib import Path

if sys.stdout and sys.stdout.encoding != "utf-8":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
if sys.stderr and sys.stderr.encoding != "utf-8":
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8", errors="replace")


def get_model_path():
    """Return path to the bundled model, or fall back to download cache."""
    # Check for bundled model next to the exe
    if getattr(sys, 'frozen', False):
        exe_dir = os.path.dirname(sys.executable)
    else:
        exe_dir = os.path.dirname(os.path.abspath(__file__))

    bundled = os.path.join(exe_dir, "model")
    if os.path.isdir(bundled) and os.path.isfile(os.path.join(bundled, "model.bin")):
        return bundled

    # Fall back to download
    home = Path.home() / ".cache" / "whisper-models"
    home.mkdir(parents=True, exist_ok=True)
    return "medium"


def transcribe(mp3_path, model_size="medium", output_file=None):
    from faster_whisper import WhisperModel

    mp3_path = os.path.abspath(mp3_path)
    if not os.path.isfile(mp3_path):
        print(f"Error: File not found: {mp3_path}")
        sys.exit(1)

    if output_file is None:
        base = os.path.splitext(mp3_path)[0]
        output_file = base + "_transcript.txt"

    model_path = get_model_path()
    is_bundled = os.path.isdir(model_path)
    print(f"Model  : {model_path if is_bundled else model_size} ({'bundled' if is_bundled else 'download'})")
    print(f"Input  : {os.path.basename(mp3_path)}")

    print("Loading model...", flush=True)
    t0 = time.time()
    if is_bundled:
        model = WhisperModel(model_path, device="cpu", compute_type="int8")
    else:
        cache = Path.home() / ".cache" / "whisper-models"
        cache.mkdir(parents=True, exist_ok=True)
        model = WhisperModel(model_size, device="cpu", compute_type="int8", download_root=str(cache))
    print(f"Model ready ({time.time() - t0:.1f}s)")

    print("Transcribing...", flush=True)
    t0 = time.time()
    segments, info = model.transcribe(
        mp3_path,
        language="tr",
        beam_size=5,
        vad_filter=True,
        vad_parameters=dict(
            min_silence_duration_ms=500,
            speech_pad_ms=300,
        ),
    )

    all_segments = list(segments)
    elapsed = time.time() - t0
    print(f"Done ({elapsed:.1f}s)")

    if not all_segments:
        with open(output_file, "w", encoding="utf-8") as f:
            f.write("No speech detected in the audio.\n")
        print(f"Output : {output_file}")
        return

    full_text = " ".join(seg.text.strip() for seg in all_segments)

    timestamped_lines = []
    for seg in all_segments:
        start_m, start_s = divmod(int(seg.start), 60)
        end_m, end_s = divmod(int(seg.end), 60)
        timestamped_lines.append(
            f"[{start_m:02d}:{start_s:02d} -> {end_m:02d}:{end_s:02d}]  {seg.text.strip()}"
        )

    with open(output_file, "w", encoding="utf-8") as f:
        f.write("=== TRANSCRIPT (Turkish) ===\n")
        f.write(f"Source: {os.path.basename(mp3_path)}\n")
        f.write(f"Model: whisper-{model_size}\n")
        f.write(f"Duration: {elapsed:.1f}s\n")
        f.write("=" * 40 + "\n\n")
        f.write(full_text)
        f.write("\n\n")
        f.write("=== TIMESTAMPED ===\n\n")
        f.write("\n".join(timestamped_lines))
        f.write("\n")

    print(f"Output : {output_file}")


def pick_file_gui():
    try:
        import tkinter as tk
        from tkinter import filedialog

        root = tk.Tk()
        root.withdraw()
        root.attributes("-topmost", True)
        filepath = filedialog.askopenfilename(
            title="Select an MP3 file to transcribe",
            filetypes=[
                ("MP3 files", "*.mp3"),
                ("Audio files", "*.mp3 *.wav *.m4a *.ogg *.flac *.wma"),
                ("All files", "*.*"),
            ],
        )
        root.destroy()
        return filepath if filepath else None
    except Exception:
        return None


def main():
    parser = argparse.ArgumentParser(
        description="Transcribe Turkish MP3 audio to text using Whisper.",
    )
    parser.add_argument("file", nargs="?", help="Path to MP3 file (opens picker if omitted)")
    parser.add_argument(
        "--model", "-m",
        default="medium",
        choices=["tiny", "base", "small", "medium", "large-v3"],
        help="Whisper model size (default: medium)",
    )
    parser.add_argument("--output", "-o", default=None, help="Output text file path")

    args = parser.parse_args()
    mp3_path = args.file

    if not mp3_path:
        mp3_path = pick_file_gui()
        if not mp3_path:
            print("No file selected.")
            sys.exit(0)

    transcribe(mp3_path, model_size=args.model, output_file=args.output)

    if len(sys.argv) == 1:
        input("Press Enter to exit...")


if __name__ == "__main__":
    main()
