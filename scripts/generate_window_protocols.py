"""Regenerate the ordinary C inputs of the Linux window Cone."""

import argparse
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROTOCOLS = ROOT / "sysroot/vendor/wayland-protocols/1.47"
OUTPUT = ROOT / "sysroot/src/scoop.window/native/linux/protocols"
SCANNER_VERSION = "wayland-scanner 1.24.0"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scanner", default="wayland-scanner")
    args = parser.parse_args()
    version = subprocess.run(
        [args.scanner, "--version"], capture_output=True, text=True, check=True
    )
    if (version.stdout + version.stderr).strip() != SCANNER_VERSION:
        parser.error(f"requires {SCANNER_VERSION}")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for source in sorted(PROTOCOLS.glob("*.xml")):
        for mode, suffix in [("client-header", ".h"), ("private-code", ".c")]:
            subprocess.run(
                [args.scanner, mode, str(source), str(OUTPUT / (source.stem + suffix))],
                check=True,
            )


if __name__ == "__main__":
    main()
