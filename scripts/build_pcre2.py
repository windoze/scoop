#!/usr/bin/env python3
"""Prepare the fixed PCRE2-8 static library for one Scoop target."""

import argparse
import fcntl
import json
import os
import platform
import shutil
import subprocess
import tarfile
import tomllib
from pathlib import Path

from build_support import RELEASE_CONFIG, ROOT, metadata, reusable, save_record

VERSION = "10.49"
TARGETS = {
    "aarch64-apple-darwin": ("Darwin", "clang"),
    "x86_64-unknown-linux-gnu": ("Linux", "gcc"),
    "x86_64-unknown-linux-musl": ("Linux", "musl-gcc"),
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
    if host == "Darwin":
        options["CMAKE_OSX_ARCHITECTURES"] = "arm64"
        options["CMAKE_OSX_DEPLOYMENT_TARGET"] = inputs["deployment"] or release["minimum_os"]
    scratch = build / "scratch"
    scratch.mkdir(exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(scratch))
    objects = build / "objects"
    subprocess.run(
        [
            cmake,
            "-G",
            "Unix Makefiles",
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
        (objects / "libpcre2-8.a", objects / "interface/pcre2.h"), outputs, strict=True
    ):
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary = output.with_suffix(output.suffix + ".new")
        shutil.copy2(built, temporary)
        temporary.replace(output)
    save_record(record, inputs, objects, tools=(cc, cmake))
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
    args.build_dir = args.build_dir.resolve()
    args.prefix = (args.prefix or ROOT / "sysroot/lib" / args.target).resolve()
    args.build_dir.mkdir(parents=True, exist_ok=True)
    with (args.build_dir / "build.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        prepare(args)


if __name__ == "__main__":
    main()
