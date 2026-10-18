#!/usr/bin/env python3
"""Prepare the fixed nghttp2 static protocol library for one Scoop target."""

import argparse
import json
import os
import platform
import shutil
import subprocess
import tarfile
import tomllib
from pathlib import Path

from build_support import RELEASE_CONFIG, ROOT, metadata, reusable, save_record

VERSION = "1.70.0"
TARGETS = {
    "aarch64-apple-darwin": ("Darwin", "clang"),
    "x86_64-unknown-linux-gnu": ("Linux", "gcc"),
    "x86_64-unknown-linux-musl": ("Linux", "musl-gcc"),
}
OPTIONS = {
    "CMAKE_BUILD_TYPE": "MinSizeRel",
    "CMAKE_INSTALL_LIBDIR": "lib",
    "CMAKE_POSITION_INDEPENDENT_CODE": "ON",
    "ENABLE_LIB_ONLY": "ON",
    "BUILD_SHARED_LIBS": "OFF",
    "BUILD_STATIC_LIBS": "ON",
    "BUILD_TESTING": "OFF",
    "ENABLE_FAILMALLOC": "OFF",
    "ENABLE_DOC": "OFF",
    "ENABLE_DEBUG": "OFF",
    "ENABLE_HTTP3": "OFF",
    "CMAKE_DISABLE_FIND_PACKAGE_OpenSSL": "ON",
    "CMAKE_DISABLE_FIND_PACKAGE_Libngtcp2": "ON",
    "CMAKE_DISABLE_FIND_PACKAGE_Libnghttp3": "ON",
    "CMAKE_DISABLE_FIND_PACKAGE_Python3": "ON",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--build-dir", type=Path)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--cc")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    host, default_cc = TARGETS[args.target]
    if platform.system() != host:
        parser.error(f"prepare {args.target} on a {host} build host")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    cc = shutil.which(args.cc or default_cc)
    cmake = shutil.which("cmake")
    if not cc or not cmake:
        parser.error("the selected C compiler and cmake must be installed")
    archive = ROOT / f"sysroot/vendor/nghttp2/nghttp2-{VERSION}.tar.gz"
    build = (args.build_dir or ROOT / "tmp/nghttp2" / args.target).resolve()
    prefix = (args.prefix or ROOT / "sysroot/lib" / args.target / "nghttp2").resolve()
    inputs = {
        "archive": metadata(archive),
        "tools": [metadata(path) for path in (cc, cmake, __file__, RELEASE_CONFIG)],
        "support": metadata(ROOT / "scripts/build_support.py"),
        "target": args.target,
        "prefix": str(prefix),
        "options": OPTIONS,
        "deployment": os.environ.get("MACOSX_DEPLOYMENT_TARGET") if host == "Darwin" else None,
    }
    record = build / "inputs.json"
    outputs = [
        prefix / name
        for name in (
            "include/nghttp2/nghttp2.h",
            "include/nghttp2/nghttp2ver.h",
            "lib/libnghttp2.a",
        )
    ]
    if reusable(record, inputs, outputs):
        print(f"Reusing {args.target} nghttp2 {VERSION}: {prefix}")
        return

    build.mkdir(parents=True, exist_ok=True)
    source = build / f"nghttp2-{VERSION}"
    source_record = build / "source.json"
    if not source_record.exists() or json.loads(source_record.read_text()) != inputs["archive"]:
        if source.exists():
            shutil.rmtree(source)
        with tarfile.open(archive) as package:
            package.extractall(build, filter="data")
        source_record.write_text(json.dumps(inputs["archive"]) + "\n")

    release = tomllib.loads(RELEASE_CONFIG.read_text())["targets"][args.target]
    options = dict(
        OPTIONS,
        CMAKE_C_COMPILER=cc,
        CMAKE_INSTALL_PREFIX=str(prefix),
        CMAKE_C_FLAGS="-mcpu=generic" if host == "Darwin" else "-march=" + release["cpu"],
    )
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
        check=True,
        env=environment,
    )
    subprocess.run(
        [
            cmake,
            "--build",
            str(objects),
            "--target",
            "nghttp2_static",
            "--parallel",
            str(args.jobs),
        ],
        check=True,
        env=environment,
    )
    subprocess.run([cmake, "--install", str(objects)], check=True, env=environment)
    save_record(record, inputs, objects)
    print(f"Prepared {args.target} nghttp2 {VERSION}: {prefix}")


if __name__ == "__main__":
    main()
