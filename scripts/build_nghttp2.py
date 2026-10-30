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

from build_support import (
    RELEASE_CONFIG,
    ROOT,
    apple_architecture,
    build_lock,
    cpu_flag,
    metadata,
    reusable,
    save_record,
)
from target_catalog import NATIVE_TARGETS as TARGETS
from target_catalog import native_configuration

VERSION = "1.70.0"

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


def prepare(args):
    host, default_cc = TARGETS[args.target]
    archive = ROOT / f"sysroot/vendor/nghttp2/nghttp2-{VERSION}.tar.gz"
    build, prefix = args.build_dir, args.prefix
    inputs = {
        "archive": metadata(archive),
        "scripts": [metadata(path) for path in (__file__, RELEASE_CONFIG)],
        "support": metadata(ROOT / "scripts/build_support.py"),
        "target": args.target,
        "target_configuration": native_configuration(args.target),
        "prefix": str(prefix),
        "cc": args.cc or default_cc,
        "options": OPTIONS,
        "environment": {
            name: os.environ.get(name)
            for name in ("PATH", "CFLAGS", "LDFLAGS", "INCLUDE", "LIB", "LIBPATH", "CL", "_CL_")
        },
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
        print(f"Reusing {args.target} nghttp2 {VERSION}: {prefix}", flush=True)
        return

    cc, cmake = shutil.which(inputs["cc"]), shutil.which("cmake")
    if not cc or not cmake:
        raise ValueError("the selected C compiler and CMake must be installed")
    ninja = shutil.which("ninja") if host == "Windows" else None
    if host == "Windows" and not ninja:
        raise ValueError("Windows nghttp2 preparation requires Ninja")
    source = build / f"nghttp2-{VERSION}"
    source_record = build / "source.json"
    if not source_record.exists() or json.loads(source_record.read_text()) != inputs["archive"]:
        if source.exists():
            shutil.rmtree(source)
        with tarfile.open(archive) as package:
            package.extractall(build, filter="data")
        source_record.write_text(json.dumps(inputs["archive"]) + "\n")

    release = tomllib.loads(RELEASE_CONFIG.read_text())["targets"][args.target]
    stage = build / "stage"
    options = dict(
        OPTIONS,
        CMAKE_C_COMPILER=cc,
        CMAKE_INSTALL_PREFIX=str(stage),
        CMAKE_C_FLAGS=cpu_flag(native_configuration(args.target)["arch"], release["cpu"]),
    )
    msvc = native_configuration(args.target)["native_environment"] == "MicrosoftUcrt"
    if host == "Windows":
        options["CMAKE_MAKE_PROGRAM"] = ninja
        options["CMAKE_SYSTEM_NAME"] = "Windows"
        options["CMAKE_POSITION_INDEPENDENT_CODE"] = "OFF"
        options["CMAKE_C_FLAGS"] += " -D_WIN32_WINNT=0x0a00"
        if msvc:
            options["CMAKE_C_FLAGS"] = "/D_WIN32_WINNT=0x0a00"
            options["CMAKE_C_COMPILER_TARGET"] = native_configuration(args.target)["llvm_triple"]
            options["CMAKE_POLICY_DEFAULT_CMP0091"] = "NEW"
            options["CMAKE_MSVC_RUNTIME_LIBRARY"] = "MultiThreadedDLL"
            options["ENABLE_STATIC_CRT"] = "OFF"
        else:
            options["CMAKE_C_COMPILER_TARGET"] = native_configuration(args.target)["llvm_triple"]
    if host == "Darwin":
        options["CMAKE_OSX_ARCHITECTURES"] = apple_architecture(
            native_configuration(args.target)["arch"]
        )
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
    (prefix / "lib").mkdir(parents=True, exist_ok=True)
    output = prefix / "lib/libnghttp2.a"
    temporary = output.with_suffix(".a.new")
    shutil.copy2(stage / "lib" / ("nghttp2.lib" if msvc else "libnghttp2.a"), temporary)
    temporary.replace(output)
    shutil.copytree(stage / "include/nghttp2", prefix / "include/nghttp2", dirs_exist_ok=True)
    tools = [cc, cmake]
    if ninja:
        tools.append(ninja)
    save_record(record, inputs, objects, tools=tools)
    print(f"Prepared {args.target} nghttp2 {VERSION}: {prefix}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--build-dir", type=Path)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--cc")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    host, _ = TARGETS[args.target]
    if platform.system() != host:
        parser.error(f"prepare {args.target} on a {host} build host")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    if "llvm-mingw" in args.target and not args.cc:
        parser.error("--cc must select clang.exe from the LLVM-MinGW prefix for this CRT")
    args.build_dir = (args.build_dir or ROOT / "tmp/nghttp2" / args.target).resolve()
    args.prefix = (args.prefix or ROOT / "sysroot/lib" / args.target / "nghttp2").resolve()
    args.build_dir.mkdir(parents=True, exist_ok=True)
    with build_lock(args.build_dir / "build.lock"):
        prepare(args)


if __name__ == "__main__":
    main()
