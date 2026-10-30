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
from compression_backends import PACKAGES
from target_catalog import NATIVE_TARGETS as TARGETS
from target_catalog import native_configuration


def prepare(package, args):
    host, default_cc = TARGETS[args.target]
    name, version = package["name"], package["version"]
    vendor = ROOT / "sysroot/vendor" / name
    archive = vendor / f"{name}-{version}.tar.gz"
    patches = sorted((vendor / "patches").glob("*.patch"))
    build, prefix = args.build_dir / name, args.prefix
    inputs = {
        "archive": metadata(archive),
        "patches": [metadata(path) for path in patches],
        "scripts": [metadata(path) for path in (__file__, RELEASE_CONFIG)],
        "support": [
            metadata(ROOT / "scripts" / file)
            for file in ("build_support.py", "compression_backends.py")
        ],
        "target": args.target,
        "target_configuration": native_configuration(args.target),
        "cc": args.cc or default_cc,
        "prefix": str(prefix),
        "package": package,
        "deployment": os.environ.get("MACOSX_DEPLOYMENT_TARGET") if host == "Darwin" else None,
        "environment": {
            name: os.environ.get(name)
            for name in (
                "PATH",
                "CFLAGS",
                "CXXFLAGS",
                "LDFLAGS",
                "INCLUDE",
                "LIB",
                "LIBPATH",
                "CL",
                "_CL_",
            )
        },
    }
    outputs = [prefix / file for file in package["libraries"]]
    outputs.append(prefix / "include" / package["header"])
    record = build / "inputs.json"
    if reusable(record, inputs, outputs):
        print(f"Reusing {args.target} {name} {version}", flush=True)
        return
    cc, cmake = shutil.which(inputs["cc"]), shutil.which("cmake")
    if not cc or not cmake:
        raise ValueError("the selected C compiler and CMake must be installed")
    tools = [cc, cmake]
    ninja = shutil.which("ninja") if host == "Windows" else None
    if host == "Windows":
        if not ninja:
            raise ValueError("Windows compression preparation requires Ninja")
        tools.append(ninja)
    patch_tool = None
    if patches:
        patch_tool = shutil.which("git" if host == "Windows" else "patch")
        if not patch_tool:
            raise ValueError("vendor patches require Git on Windows or patch on Unix")
        tools.append(patch_tool)
    build.mkdir(parents=True, exist_ok=True)
    source = build / "source"
    source_record = build / "source.json"
    source_inputs = {key: inputs[key] for key in ("archive", "patches")}
    if not source_record.exists() or json.loads(source_record.read_text()) != source_inputs:
        if source.exists():
            shutil.rmtree(source)
        with tarfile.open(archive) as tar:
            top = tar.getmembers()[0].name.split("/")[0]
            tar.extractall(build, filter="data")
        (build / top).rename(source)
        for patch in patches:
            command = (
                [patch_tool, "apply", "-"]
                if host == "Windows"
                else [patch_tool, "--batch", "-p1", "-i", str(patch)]
            )
            # A Windows checkout can use CRLF; the vendor archive retains LF.
            patch_input = (
                patch.read_text(encoding="utf-8").encode("utf-8") if host == "Windows" else None
            )
            subprocess.run(command, input=patch_input, cwd=source, check=True)
        source_record.write_text(json.dumps(source_inputs) + "\n")
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
        CMAKE_INSTALL_PREFIX=str(stage),
        CMAKE_INSTALL_LIBDIR="lib",
        CMAKE_BUILD_TYPE="MinSizeRel",
        CMAKE_POSITION_INDEPENDENT_CODE="ON",
        BUILD_SHARED_LIBS="OFF",
        BUILD_TESTING="OFF",
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
        else:
            options["CMAKE_C_COMPILER_TARGET"] = native_configuration(args.target)["llvm_triple"]
        if name == "zstd":
            cxx = cc if msvc else str(Path(cc).with_name("clang++.exe"))
            options["CMAKE_CXX_COMPILER"] = cxx
            options["CMAKE_CXX_COMPILER_TARGET"] = options["CMAKE_C_COMPILER_TARGET"]
            options["CMAKE_CXX_FLAGS"] = options["CMAKE_C_FLAGS"]
            options["CMAKE_ASM_COMPILER"] = cc
            if msvc:
                options["ZSTD_USE_STATIC_RUNTIME"] = "OFF"
            tools.append(cxx)
    if darwin:
        options["CMAKE_OSX_ARCHITECTURES"] = apple_architecture(
            native_configuration(args.target)["arch"]
        )
        options["CMAKE_OSX_DEPLOYMENT_TARGET"] = inputs["deployment"] or release["minimum_os"]
    environment = dict(os.environ, TMPDIR=str(scratch), TEMP=str(scratch), TMP=str(scratch))
    if host == "Windows":
        environment["PATH"] = str(Path(cc).parent) + os.pathsep + environment["PATH"]
    subprocess.run(
        [
            cmake,
            "-G",
            "Ninja" if host == "Windows" else "Unix Makefiles",
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
    subprocess.run([cmake, "--install", str(objects)], env=environment, check=True)
    prefix.mkdir(parents=True, exist_ok=True)
    built = package["msvc_libraries"] if msvc else package["libraries"]
    for source_name, output_name in zip(built, package["libraries"], strict=True):
        output = prefix / output_name
        temporary = output.with_suffix(output.suffix + ".new")
        shutil.copy2(stage / "lib" / source_name, temporary)
        temporary.replace(output)
    shutil.copytree(stage / "include", prefix / "include", dirs_exist_ok=True)
    save_record(record, inputs, objects, tools=tools)
    print(f"Prepared {args.target} {name} {version}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--build-dir", type=Path, required=True)
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
    args.build_dir = args.build_dir.resolve()
    args.prefix = (args.prefix or ROOT / "sysroot/lib" / args.target).resolve()
    args.build_dir.mkdir(parents=True, exist_ok=True)
    with build_lock(args.build_dir / "build.lock"):
        for package in PACKAGES:
            prepare(package, args)


if __name__ == "__main__":
    main()
