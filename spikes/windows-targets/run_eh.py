#!/usr/bin/env python3
"""Run W0 SEH probes on Windows with an explicitly selected native environment."""

import argparse
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


def run(command, *, stdout=None):
    print(subprocess.list2cmdline([str(arg) for arg in command]), flush=True)
    subprocess.run(command, check=True, stdout=stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--target", choices=("msvc-ucrt", "mingw-ucrt", "mingw-msvcrt"), required=True
    )
    parser.add_argument("--llvm", type=Path, required=True, help="LLVM or LLVM-MinGW prefix")
    parser.add_argument("--unwind", type=Path, required=True, help="matching libunwind prefix")
    parser.add_argument("--build-dir", type=Path, required=True)
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("execute these W0 probes on a real Windows runner")
    build = args.build_dir.resolve()
    build.mkdir(parents=True, exist_ok=True)
    llvm = args.llvm.resolve()
    unwind = args.unwind.resolve()
    msvc = args.target == "msvc-ucrt"
    cc = llvm / "bin/clang.exe"
    triple = "x86_64-pc-windows-msvc" if msvc else "x86_64-w64-windows-gnu"
    flags = [
        "-target",
        triple,
        "-std=c11",
        "-O2",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-Wno-override-module",
        "-D__SEH__",
        "-I",
        unwind / "include",
        "-I",
        ROOT / "runtime/src",
    ]
    if msvc:
        flags += ["-fms-runtime-lib=dll", "-fuse-ld=lld"]
    decoder = ROOT / "runtime/src/eh/lsda.c"
    personality = ROOT / "runtime/src/eh_personality.c"
    decoder_test = ROOT / "runtime/tests/eh_decoder_test.c"
    # Compile diagnostics before running the probes.
    run([cc, *flags, "-fsyntax-only", decoder, personality, decoder_test, HERE / "eh.c"])
    run([cc, *flags, decoder, decoder_test, "-o", build / "decoder.exe"])
    run([build / "decoder.exe"])
    library = unwind / "lib" / ("unwind.lib" if msvc else "libunwind.a")
    run(
        [
            cc,
            *flags,
            decoder,
            personality,
            HERE / "eh.c",
            HERE / "eh.ll",
            library,
            "-o",
            build / "eh.exe",
        ]
    )
    run([build / "eh.exe"])
    with (build / "pe.txt").open("w") as output:
        run(
            [llvm / "bin/llvm-readobj.exe", "--coff-imports", "--unwind", build / "eh.exe"],
            stdout=output,
        )
    imports = (build / "pe.txt").read_text().lower()
    old_crt = "name: msvcrt.dll" in imports
    ucrt = "ucrtbase.dll" in imports or "api-ms-win-crt-" in imports
    if old_crt != (args.target == "mingw-msvcrt") or ucrt == old_crt:
        raise RuntimeError(f"unexpected CRT imports for {args.target}; see {build / 'pe.txt'}")
    print(f"{args.target}: real SEH catch/cleanup/resume and CRT imports passed", flush=True)


if __name__ == "__main__":
    main()
