#!/usr/bin/env python3
"""W0: relocate a real Win64 stackmap root using the system frame decoder."""

import argparse
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--llvm", type=Path, required=True, help="selected LLVM/LLVM-MinGW prefix")
    parser.add_argument("--build-dir", type=Path, required=True)
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("this probe needs the real Windows RtlVirtualUnwind")
    build = args.build_dir.resolve()
    build.mkdir(parents=True, exist_ok=True)
    cc = args.llvm.resolve() / "bin/clang.exe"
    flags = [
        "-std=c11",
        "-O2",
        "-I",
        ROOT / "runtime/src",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-Wno-override-module",
        "-fuse-ld=lld",
    ]
    sources = [HERE / "roots.c", ROOT / "runtime/src/gc_binding/stackmap.c"]
    sources += sorted((ROOT / "runtime/src/gc_binding/stackmap").glob("*.c"))
    subprocess.run([cc, *flags, "-fsyntax-only", *sources], check=True)
    for frame_pointer in ("all", "none"):
        ir = build / (frame_pointer + ".ll")
        ir.write_text(
            (HERE / "roots.ll")
            .read_text()
            .replace('"frame-pointer"="all"', f'"frame-pointer"="{frame_pointer}"')
        )
        executable = build / (frame_pointer + ".exe")
        subprocess.run([cc, *flags, *sources, ir, "-o", executable], check=True)
        subprocess.run([executable], check=True)


if __name__ == "__main__":
    main()
