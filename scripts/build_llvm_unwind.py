#!/usr/bin/env python3
"""Build a private LLVM libunwind archive for a supported Linux target."""

import argparse
import os
import shlex
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGETS = ("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--source", type=Path, default=ROOT.parent / "llvm-project-22.1.2.src")
    parser.add_argument("--build-dir", type=Path)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--cc", default="clang", help="Clang C/assembly compiler")
    parser.add_argument("--cxx", default="clang++", help="Clang C++ compiler")
    parser.add_argument("--musl-include", type=Path, help="musl's installed C header directory")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 8))
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    source = args.source.resolve()
    for relative in ("runtimes/CMakeLists.txt", "libunwind/include/unwind.h"):
        if not (source / relative).is_file():
            parser.error(f"LLVM source is missing {relative}: {source}")
    cc, cxx = shutil.which(args.cc), shutil.which(args.cxx)
    if not cc or not cxx:
        parser.error("--cc and --cxx must name installed Clang compilers")
    musl = args.target.endswith("-musl")
    if musl and (args.musl_include is None or not (args.musl_include / "features.h").is_file()):
        parser.error("musl requires --musl-include pointing to its installed C headers")
    if not musl and args.musl_include is not None:
        parser.error("--musl-include is only valid for the musl target")

    build = (args.build_dir or ROOT / "target/llvm-unwind" / args.target).resolve()
    prefix = (args.prefix or ROOT / "sysroot/native" / args.target / "unwind").resolve()
    temporary = build / "tmp"
    temporary.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(temporary))
    configure = [
        "cmake",
        "-G",
        "Unix Makefiles",
        "-S",
        str(source / "runtimes"),
        "-B",
        str(build),
        "-DLLVM_ENABLE_RUNTIMES=libunwind",
        "-DCMAKE_BUILD_TYPE=Release",
        "-DLLVM_INCLUDE_TESTS=OFF",
        "-DLLVM_INCLUDE_DOCS=OFF",
        "-DLIBUNWIND_INCLUDE_TESTS=OFF",
        "-DLIBUNWIND_INCLUDE_DOCS=OFF",
        "-DLIBUNWIND_ENABLE_SHARED=OFF",
        "-DLIBUNWIND_ENABLE_STATIC=ON",
        "-DLIBUNWIND_ENABLE_THREADS=ON",
        "-DLIBUNWIND_ENABLE_CROSS_UNWINDING=OFF",
        "-DLIBUNWIND_HIDE_SYMBOLS=ON",
        "-DCMAKE_POSITION_INDEPENDENT_CODE=ON",
        "-DCMAKE_TRY_COMPILE_TARGET_TYPE=STATIC_LIBRARY",
        "-DLIBUNWIND_INSTALL_LIBRARY_DIR=lib",
        "-DLIBUNWIND_INSTALL_INCLUDE_DIR=include",
        f"-DCMAKE_INSTALL_PREFIX={prefix}",
    ]
    for language, compiler in (("C", cc), ("CXX", cxx), ("ASM", cc)):
        configure.extend(
            [
                f"-DCMAKE_{language}_COMPILER={compiler}",
                f"-DCMAKE_{language}_COMPILER_TARGET={args.target}",
            ]
        )
    if musl:
        resource = subprocess.check_output([cc, "-print-resource-dir"], text=True).strip()
        flags = [
            "-nostdinc",
            "-isystem",
            str(Path(resource) / "include"),
            "-isystem",
            str(args.musl_include.resolve()),
        ]
        for language in ("C", "CXX", "ASM"):
            selected = flags + (["-nostdinc++"] if language == "CXX" else [])
            configure.append(f"-DCMAKE_{language}_FLAGS={shlex.join(selected)}")
    subprocess.run(configure, check=True, env=environment)
    subprocess.run(
        [
            "cmake",
            "--build",
            str(build),
            "--target",
            "install-unwind",
            "--parallel",
            str(args.jobs),
        ],
        check=True,
        env=environment,
    )
    print(f"Installed {args.target} LLVM libunwind in {prefix}")
    print("Verify the target libc with the EH link/run test before using this archive.")


if __name__ == "__main__":
    main()
