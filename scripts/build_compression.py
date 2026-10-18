#!/usr/bin/env python3
"""Prepare the fixed zstd, zlib-ng and Brotli static libraries for one target."""

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
from compression_backends import PACKAGES

TARGETS = {
    "aarch64-apple-darwin": ("Darwin", "clang"),
    "x86_64-unknown-linux-gnu": ("Linux", "gcc"),
    "x86_64-unknown-linux-musl": ("Linux", "musl-gcc"),
}


def prepare(package, args, cc, cmake):
    name, version = package["name"], package["version"]
    archive = ROOT / "sysroot/vendor" / name / f"{name}-{version}.tar.gz"
    build, prefix = args.build_dir / name, args.prefix
    inputs = {
        "archive": metadata(archive),
        "tools": [metadata(path) for path in (cc, cmake, __file__, RELEASE_CONFIG)],
        "support": [
            metadata(ROOT / "scripts" / file)
            for file in ("build_support.py", "compression_backends.py")
        ],
        "target": args.target,
        "prefix": str(prefix),
        "package": package,
        "deployment": os.environ.get("MACOSX_DEPLOYMENT_TARGET"),
    }
    outputs = [prefix / file for file in package["libraries"]]
    outputs.append(prefix / "include" / package["header"])
    record = build / "inputs.json"
    if reusable(record, inputs, outputs):
        print(f"Reusing {args.target} {name} {version}", flush=True)
        return
    build.mkdir(parents=True, exist_ok=True)
    source = build / "source"
    source_record = build / "source.json"
    if not source_record.exists() or json.loads(source_record.read_text()) != inputs["archive"]:
        if source.exists():
            shutil.rmtree(source)
        with tarfile.open(archive) as tar:
            top = tar.getmembers()[0].name.split("/")[0]
            tar.extractall(build, filter="data")
        (build / top).rename(source)
        source_record.write_text(json.dumps(inputs["archive"]) + "\n")
    objects, stage, scratch = (build / part for part in ("objects", "stage", "scratch"))
    for directory in (objects, stage):
        if directory.exists():
            shutil.rmtree(directory)
        directory.mkdir()
    scratch.mkdir(exist_ok=True)
    release = tomllib.loads(RELEASE_CONFIG.read_text())["targets"][args.target]
    darwin = args.target.endswith("darwin")
    options = dict(
        package["options"],
        CMAKE_C_COMPILER=cc,
        CMAKE_INSTALL_PREFIX=str(prefix),
        CMAKE_INSTALL_LIBDIR=str(prefix),
        CMAKE_BUILD_TYPE="MinSizeRel",
        CMAKE_POSITION_INDEPENDENT_CODE="ON",
        BUILD_SHARED_LIBS="OFF",
        BUILD_TESTING="OFF",
        CMAKE_C_FLAGS="-mcpu=generic" if darwin else "-march=" + release["cpu"],
    )
    if darwin:
        options["CMAKE_OSX_ARCHITECTURES"] = "arm64"
        options["CMAKE_OSX_DEPLOYMENT_TARGET"] = inputs["deployment"] or release["minimum_os"]
    environment = dict(os.environ, TMPDIR=str(scratch))
    subprocess.run(
        [
            cmake,
            "-G",
            "Unix Makefiles",
            "-S",
            str(source / package["source"]),
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
            *package["targets"],
            "--parallel",
            str(args.jobs),
        ],
        env=environment,
        check=True,
    )
    subprocess.run(
        [cmake, "--install", str(objects)], env=dict(environment, DESTDIR=str(stage)), check=True
    )
    installed = stage / prefix.relative_to(prefix.anchor)
    prefix.mkdir(parents=True, exist_ok=True)
    for file in package["libraries"]:
        shutil.copy2(installed / file, prefix / file)
    shutil.copytree(installed / "include", prefix / "include", dirs_exist_ok=True)
    save_record(record, inputs, objects)
    print(f"Prepared {args.target} {name} {version}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--build-dir", type=Path, required=True)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--cc")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    host, default_cc = TARGETS[args.target]
    if platform.system() != host:
        parser.error(f"prepare {args.target} on a {host} build host")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    cc, cmake = shutil.which(args.cc or default_cc), shutil.which("cmake")
    if not cc or not cmake:
        parser.error("the selected C compiler and CMake must be installed")
    args.build_dir = args.build_dir.resolve()
    args.prefix = (args.prefix or ROOT / "sysroot/lib" / args.target).resolve()
    for package in PACKAGES:
        prepare(package, args, cc, cmake)


if __name__ == "__main__":
    main()
