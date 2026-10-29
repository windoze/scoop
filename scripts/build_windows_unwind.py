#!/usr/bin/env python3
"""Build static LLVM Level-I SEH support for a Windows target."""

import argparse
import os
import shutil
import subprocess
from pathlib import Path

from build_support import (
    RELEASE_CONFIG,
    ROOT,
    build_lock,
    metadata,
    reusable,
    save_record,
    source_metadata,
)
from prepare_sysroot import publish_files

TARGETS = (
    "x86_64-pc-windows-msvc-ucrt",
    "x86_64-w64-windows-llvm-mingw-msvcrt",
    "x86_64-w64-windows-llvm-mingw-ucrt",
)
PATCH = ROOT / "sysroot/vendor/llvm-project/22.1.8/patches/llvm-libunwind-msvc-seh.patch"
SOURCE_DIRECTORIES = (
    "libunwind",
    "runtimes",
    "cmake",
    "llvm/cmake",
    "llvm/include",
    "third-party",
)


def prepare(args, build, prefix):
    microsoft = args.target == TARGETS[0]
    source = args.source.resolve()
    inputs = {
        "source": source_metadata(source, SOURCE_DIRECTORIES),
        "scripts": [
            metadata(path)
            for path in (
                __file__,
                ROOT / "scripts/build_support.py",
                ROOT / "scripts/prepare_sysroot.py",
                PATCH,
                RELEASE_CONFIG,
            )
        ],
        "target": args.target,
        "prefix": str(prefix),
        "cc": args.cc,
        "environment": {
            key: os.environ.get(key)
            for key in ("PATH", "INCLUDE", "LIB", "VCToolsInstallDir", "WindowsSdkDir")
        },
    }
    record = build / "inputs.json"
    if reusable(record, inputs, [prefix / "include/unwind.h", prefix / "lib/libunwind.a"]):
        print(f"Reusing {args.target} LLVM libunwind: {prefix}", flush=True)
        return

    cc = shutil.which(args.cc)
    if cc is None:
        raise FileNotFoundError(f"Clang compiler not found: {args.cc}")
    cc = Path(cc).resolve()
    if microsoft:
        cc = cc.with_name("clang-cl" + cc.suffix)
        if not cc.is_file():
            raise FileNotFoundError(f"Clang MSVC driver not found: {cc}")
    cxx = cc if microsoft else cc.with_name("clang++" + cc.suffix)
    if not cxx.is_file():
        raise FileNotFoundError(f"Clang C++ compiler not found: {cxx}")
    scratch = build / "scratch"
    scratch.mkdir(exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(scratch), TMP=str(scratch), TEMP=str(scratch))
    environment["PATH"] = str(cc.parent) + os.pathsep + environment.get("PATH", "")
    tools = [cc, cxx]
    for name in ("cmake", "ninja"):
        tool = shutil.which(name, path=environment["PATH"])
        if tool is None:
            raise FileNotFoundError(f"Required build tool not found: {name}")
        tools.append(Path(tool))

    overlay = (build / "source").resolve()
    if not overlay.is_relative_to(build):
        raise ValueError(f"Source overlay must stay inside the build directory: {overlay}")
    if overlay.exists():
        shutil.rmtree(overlay)
    for directory in SOURCE_DIRECTORIES:
        shutil.copytree(source / directory, overlay / directory)
    if microsoft:
        subprocess.run(
            ["git", "apply", "--whitespace=nowarn", "-"],
            input=PATCH.read_text(encoding="utf-8").encode("utf-8"),
            cwd=overlay,
            env=environment,
            check=True,
        )

    objects = build / "objects"
    stage = build / "stage"
    triple = "x86_64-pc-windows-msvc" if microsoft else "x86_64-w64-windows-gnu"
    configure = [
        str(tools[2]),
        "-G",
        "Ninja",
        "-S",
        str(overlay / "runtimes"),
        "-B",
        str(objects),
        "-DCMAKE_SYSTEM_NAME=Windows",
        "-DCMAKE_SYSTEM_PROCESSOR=AMD64",
        "-DCMAKE_BUILD_TYPE=Release",
        "-DCMAKE_POLICY_DEFAULT_CMP0091=NEW",
        "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL",
        "-DLLVM_ENABLE_RUNTIMES=libunwind",
        f"-DLLVM_DEFAULT_TARGET_TRIPLE={triple}",
        "-DLLVM_INCLUDE_TESTS=OFF",
        "-DLLVM_INCLUDE_DOCS=OFF",
        "-DLIBUNWIND_INCLUDE_TESTS=OFF",
        "-DLIBUNWIND_INCLUDE_DOCS=OFF",
        "-DLIBUNWIND_ENABLE_SHARED=OFF",
        "-DLIBUNWIND_ENABLE_STATIC=ON",
        "-DLIBUNWIND_ENABLE_CROSS_UNWINDING=OFF",
        "-DLIBUNWIND_ENABLE_THREADS=ON",
        "-DLIBUNWIND_HIDE_SYMBOLS=ON",
        "-DLIBUNWIND_INSTALL_LIBRARY_DIR=lib",
        "-DLIBUNWIND_INSTALL_INCLUDE_DIR=include",
        f"-DCMAKE_INSTALL_PREFIX={stage.as_posix()}",
    ]
    if microsoft:
        configure.extend(
            ["-DLIBUNWIND_ENABLE_MSVC_SEH=ON", "-DLIBUNWIND_SHARED_OUTPUT_NAME=unwind_shared"]
        )
    else:
        configure.append("-DLIBUNWIND_USE_COMPILER_RT=ON")
    for language, compiler in (("C", cc), ("CXX", cxx), ("ASM", cc)):
        configure.extend(
            [
                f"-DCMAKE_{language}_COMPILER={compiler.as_posix()}",
                f"-DCMAKE_{language}_COMPILER_TARGET={triple}",
                f"-DCMAKE_{language}_FLAGS="
                + ("/clang:-march=x86-64" if microsoft else "-march=x86-64"),
            ]
        )
    subprocess.run(configure, env=environment, check=True)
    subprocess.run(
        [
            str(tools[2]),
            "--build",
            str(objects),
            "--target",
            "install-unwind",
            "install-unwind-headers",
            "--parallel",
            str(args.jobs),
        ],
        env=environment,
        check=True,
    )
    canonical = stage / "lib/libunwind.a"
    if microsoft and (stage / "lib/unwind.lib").is_file():
        shutil.copy2(stage / "lib/unwind.lib", canonical)
    files = [Path("lib/libunwind.a")]
    files.extend(
        sorted(path.relative_to(stage) for path in (stage / "include").rglob("*") if path.is_file())
    )
    publish_files(stage, prefix, files, build / "published.json")
    save_record(record, inputs, objects, tools)
    print(f"Prepared {args.target} LLVM libunwind: {prefix}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--source", type=Path, required=True, help="LLVM 22.1.8 source root")
    parser.add_argument("--cc", required=True, help="Clang from the selected native environment")
    parser.add_argument("--build-dir", type=Path)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    build = (args.build_dir or ROOT / "tmp/windows-unwind" / args.target).resolve()
    prefix = (args.prefix or ROOT / "sysroot/lib" / args.target / "unwind").resolve()
    build.mkdir(parents=True, exist_ok=True)
    with build_lock(build / "build.lock"):
        prepare(args, build, prefix)


if __name__ == "__main__":
    main()
