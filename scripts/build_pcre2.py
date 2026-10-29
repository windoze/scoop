#!/usr/bin/env python3
"""Prepare the fixed PCRE2-8 static library for one Scoop target."""

import argparse
import json
import os
import platform
import shutil
import subprocess
import tarfile
import tomllib
from pathlib import Path

from build_support import RELEASE_CONFIG, ROOT, build_lock, metadata, reusable, save_record

VERSION = "10.49"
TARGETS = {
    "aarch64-apple-darwin": ("Darwin", "clang"),
    "x86_64-unknown-linux-gnu": ("Linux", "gcc"),
    "x86_64-unknown-linux-musl": ("Linux", "musl-gcc"),
    "x86_64-pc-windows-msvc-ucrt": ("Windows", "clang-cl"),
    "x86_64-w64-windows-llvm-mingw-msvcrt": ("Windows", "clang"),
    "x86_64-w64-windows-llvm-mingw-ucrt": ("Windows", "clang"),
}
OPTIONS = {
    "CMAKE_BUILD_TYPE": "Release",
    "BUILD_SHARED_LIBS": "OFF",
    "BUILD_STATIC_LIBS": "ON",
    "PCRE2_STATIC_PIC": "ON",
    "PCRE2_BUILD_PCRE2_8": "ON",
    "PCRE2_BUILD_PCRE2_16": "OFF",
    "PCRE2_BUILD_PCRE2_32": "OFF",
    "PCRE2_BUILD_PCRE2GREP": "OFF",
    "PCRE2_BUILD_TESTS": "OFF",
    "PCRE2_SUPPORT_JIT": "ON",
    "PCRE2_SUPPORT_UNICODE": "ON",
}


def prepare(args):
    archive = ROOT / f"sysroot/vendor/pcre2/pcre2-{VERSION}.tar.bz2"
    build = args.build_dir
    prefix = args.prefix
    host, default_cc = TARGETS[args.target]
    inputs = {
        "archive": metadata(archive),
        "scripts": [
            metadata(path) for path in (__file__, RELEASE_CONFIG, ROOT / "scripts/build_support.py")
        ],
        "target": args.target,
        "prefix": str(prefix),
        "cc": args.cc or default_cc,
        "options": OPTIONS,
        "path": os.environ.get("PATH"),
        "msvc_environment": {
            name: os.environ.get(name) for name in ("INCLUDE", "LIB", "LIBPATH", "CL", "_CL_")
        }
        if host == "Windows"
        else None,
        "deployment": os.environ.get("MACOSX_DEPLOYMENT_TARGET") if host == "Darwin" else None,
    }
    record = build / "inputs.json"
    outputs = [prefix / "libpcre2-8.a", prefix / "include/pcre2.h"]
    if reusable(record, inputs, outputs):
        print(f"Reusing {args.target} PCRE2 {VERSION}: {prefix}", flush=True)
        return

    cc = shutil.which(inputs["cc"])
    cmake = shutil.which("cmake")
    if not cc or not cmake:
        raise ValueError("the selected C compiler and cmake must be installed")
    ninja = shutil.which("ninja") if host == "Windows" else None
    if host == "Windows" and not ninja:
        raise ValueError("Windows PCRE2 preparation requires Ninja")
    source = build / f"pcre2-{VERSION}"
    source_record = build / "source.json"
    if not source_record.exists() or json.loads(source_record.read_text()) != inputs["archive"]:
        if source.exists():
            shutil.rmtree(source)
        with tarfile.open(archive) as package:
            package.extractall(build, filter="data")
        source_record.write_text(json.dumps(inputs["archive"]) + "\n")

    release = tomllib.loads(RELEASE_CONFIG.read_text())["targets"][args.target]
    options = dict(OPTIONS, CMAKE_C_COMPILER=cc)
    options["CMAKE_C_FLAGS"] = "-mcpu=generic" if host == "Darwin" else "-march=" + release["cpu"]
    msvc = args.target == "x86_64-pc-windows-msvc-ucrt"
    if host == "Windows":
        options["CMAKE_MAKE_PROGRAM"] = ninja
        options["CMAKE_SYSTEM_NAME"] = "Windows"
        options["PCRE2_STATIC_PIC"] = "OFF"
        options["CMAKE_C_FLAGS"] += " -D_WIN32_WINNT=0x0a00"
        if msvc:
            options["CMAKE_C_FLAGS"] = "/D_WIN32_WINNT=0x0a00"
            options["CMAKE_C_COMPILER_TARGET"] = "x86_64-pc-windows-msvc"
            options["CMAKE_MSVC_RUNTIME_LIBRARY"] = "MultiThreadedDLL"
        else:
            options["CMAKE_C_COMPILER_TARGET"] = "x86_64-w64-windows-gnu"
    if host == "Darwin":
        options["CMAKE_OSX_ARCHITECTURES"] = "arm64"
        options["CMAKE_OSX_DEPLOYMENT_TARGET"] = inputs["deployment"] or release["minimum_os"]
    scratch = build / "scratch"
    scratch.mkdir(exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(scratch), TEMP=str(scratch), TMP=str(scratch))
    if host == "Windows":
        environment["PATH"] = str(Path(cc).parent) + os.pathsep + environment["PATH"]
    objects = build / "objects"
    subprocess.run(
        [
            cmake,
            "-G",
            "Ninja" if host == "Windows" else "Unix Makefiles",
            "-S",
            str(source),
            "-B",
            str(objects),
            *(f"-D{key}={value}" for key, value in options.items()),
        ],
        env=environment,
        check=True,
    )
    subprocess.run(
        [
            cmake,
            "--build",
            str(objects),
            "--target",
            "pcre2-8-static",
            "--parallel",
            str(args.jobs),
        ],
        env=environment,
        check=True,
    )
    for built, output in zip(
        (
            objects / ("pcre2-8-static.lib" if msvc else "libpcre2-8.a"),
            objects / "interface/pcre2.h",
        ),
        outputs,
        strict=True,
    ):
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary = output.with_suffix(output.suffix + ".new")
        shutil.copy2(built, temporary)
        temporary.replace(output)
    tools = [cc, cmake]
    if ninja:
        tools.append(ninja)
    save_record(record, inputs, objects, tools=tools)
    print(f"Prepared {args.target} PCRE2 {VERSION}: {prefix}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--build-dir", required=True, type=Path)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--cc")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    if platform.system() != TARGETS[args.target][0]:
        parser.error(f"prepare {args.target} on a {TARGETS[args.target][0]} build host")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    if "llvm-mingw" in args.target and not args.cc:
        parser.error("--cc must select clang.exe from the LLVM-MinGW prefix for this CRT")
    args.build_dir = args.build_dir.resolve()
    args.prefix = (args.prefix or ROOT / "sysroot/lib" / args.target).resolve()
    args.build_dir.mkdir(parents=True, exist_ok=True)
    with build_lock(args.build_dir / "build.lock"):
        prepare(args)


if __name__ == "__main__":
    main()
