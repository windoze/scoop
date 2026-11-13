"""Build and install the PGO command, using the paired Scoop driver and LLVM tools at run time."""

import argparse
import os
import shutil
import subprocess
from pathlib import Path

from build_support import ROOT


def install(binary, sysroot):
    commands = sysroot / "commands"
    commands.mkdir(parents=True, exist_ok=True)
    shutil.copy2(binary, commands / binary.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sysroot", type=Path, default=ROOT / "sysroot")
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target")
    args = parser.parse_args()
    output = args.target_dir.resolve()
    subprocess.run(
        ["cargo", "build", "--release", "--locked", "-p", "scoop-pgo", "--target-dir", str(output)],
        cwd=ROOT,
        check=True,
    )
    target = os.environ.get("CARGO_BUILD_TARGET")
    if target:
        output /= target
    suffix = ".exe" if os.name == "nt" else ""
    install(output / "release" / ("scoop-pgo" + suffix), args.sysroot.resolve())


if __name__ == "__main__":
    main()
