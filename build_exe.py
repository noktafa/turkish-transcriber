"""
Build standalone .exe using PyInstaller.

Usage:
    python build_exe.py

This produces: dist/transcriber.exe
"""

import subprocess
import sys


def main():
    cmd = [
        sys.executable, "-m", "PyInstaller",
        "--onefile",
        "--name", "transcriber",
        "--console",
        "--icon", "NONE",
        # Include faster_whisper and its transitive deps
        "--collect-all", "faster_whisper",
        "--collect-all", "ctranslate2",
        "--hidden-import", "tkinter",
        "transcriber.py",
    ]

    print("Building executable...")
    print(f"Command: {' '.join(cmd)}")
    print()

    result = subprocess.run(cmd)
    if result.returncode == 0:
        print()
        print("=" * 50)
        print("  Build successful!")
        print("  Output: dist/transcriber.exe")
        print("=" * 50)
    else:
        print()
        print("Build failed. Make sure PyInstaller is installed:")
        print("  pip install pyinstaller")
        sys.exit(1)


if __name__ == "__main__":
    main()
