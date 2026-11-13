#!/usr/bin/env python3
"""Build the target LLVM compiler-rt profile archive for Scoop profiling."""

import argparse
import os
import shlex
import shutil
import subprocess
import tomllib
from pathlib import Path

from build_support import (
    RELEASE_CONFIG,
    ROOT,
    apple_architecture,
    build_lock,
    configure_cmake,
    cpu_flag,
    metadata,
    reusable,
    save_record,
    source_metadata,
)
from target_catalog import TARGETS, native_configuration

SOURCE_PARTS = (
    "compiler-rt/lib/profile",
    "compiler-rt/include",
    "compiler-rt/cmake",
    "runtimes",
    "cmake",
    "llvm/cmake",
    "llvm/include",
    "third-party",
)


def prepare(args, build, output):
    target = native_configuration(args.target)
    microsoft = target["native_environment"] == "MicrosoftUcrt"
    source = args.source.resolve()
    inputs = {
        "source": source_metadata(source, SOURCE_PARTS),
        "files": [
            metadata(path)
            for path in (
                source / "compiler-rt/CMakeLists.txt",
                source / "compiler-rt/lib/CMakeLists.txt",
                __file__,
                ROOT / "scripts/build_support.py",
                RELEASE_CONFIG,
            )
        ],
        "target": target,
        "output": str(output),
        "cc": args.cc,
        "musl_include": str(args.musl_include) if args.musl_include else None,
        "environment": {
            key: os.environ.get(key)
            for key in ("PATH", "INCLUDE", "LIB", "VCToolsInstallDir", "WindowsSdkDir")
        },
    }
    record = build / "inputs.json"
    if reusable(record, inputs, [output]):
        print(f"Reusing {args.target} compiler-rt profile: {output}", flush=True)
        return

    cc = shutil.which(args.cc)
    if cc is None:
        raise FileNotFoundError(f"Clang compiler not found: {args.cc}")
    cc = Path(cc).absolute()
    cxx = cc if microsoft else cc.with_name("clang++" + cc.suffix)
    if not cxx.is_file():
        raise FileNotFoundError(f"Clang C++ compiler not found: {cxx}")
    tools = [cc, cxx]
    for name in ("cmake", "ninja"):
        tool = shutil.which(name)
        if tool is None:
            raise FileNotFoundError(f"Required build tool not found: {name}")
        tools.append(Path(tool))
    if microsoft:
        # LLVM 22 forces /MT in this directory. Scoop's MSVC target uses /MD.
        # Patch only the private build input, keeping the supplied source intact.
        overlay = build / "source"
        if overlay.exists():
            shutil.rmtree(overlay)
        for part in (
            "compiler-rt",
            "runtimes",
            "cmake",
            "llvm/cmake",
            "llvm/include",
            "third-party",
        ):
            shutil.copytree(source / part, overlay / part)
        cmake = overlay / "compiler-rt/lib/profile/CMakeLists.txt"
        original = cmake.read_text()
        old = "set(CMAKE_MSVC_RUNTIME_LIBRARY MultiThreaded)"
        if original.count(old) != 1:
            raise ValueError("LLVM profile runtime no longer has the expected MSVC CRT selection")
        cmake.write_text(original.replace(old, "set(CMAKE_MSVC_RUNTIME_LIBRARY MultiThreadedDLL)"))
        source = overlay

    release = tomllib.loads(RELEASE_CONFIG.read_text())["targets"][args.target]
    flag = cpu_flag(target["arch"], release["cpu"])
    options = {
        "CMAKE_BUILD_TYPE": "Release",
        "LLVM_ENABLE_RUNTIMES": "compiler-rt",
        "LLVM_INCLUDE_TESTS": "OFF",
        "COMPILER_RT_INCLUDE_TESTS": "OFF",
        "COMPILER_RT_DEFAULT_TARGET_ONLY": "ON",
        "LLVM_DEFAULT_TARGET_TRIPLE": target["llvm_triple"],
        "CMAKE_POSITION_INDEPENDENT_CODE": "ON",
        "CMAKE_TRY_COMPILE_TARGET_TYPE": "STATIC_LIBRARY",
    }
    for component in (
        "BUILTINS",
        "SANITIZERS",
        "XRAY",
        "LIBFUZZER",
        "CTX_PROFILE",
        "MEMPROF",
        "ORC",
        "GWP_ASAN",
    ):
        options[f"COMPILER_RT_BUILD_{component}"] = "OFF"
    options["COMPILER_RT_BUILD_PROFILE"] = "ON"
    flags = [flag]
    if args.musl_include:
        resource = subprocess.check_output([cc, "-print-resource-dir"], text=True).strip()
        flags += [
            "-nostdinc",
            "-isystem",
            str(Path(resource) / "include"),
            "-isystem",
            str(args.musl_include),
        ]
    if target["os"] == "darwin":
        architecture = apple_architecture(target["arch"])
        options.update(
            {
                "CMAKE_OSX_ARCHITECTURES": architecture,
                "DARWIN_osx_ARCHS": architecture,
                "CMAKE_OSX_DEPLOYMENT_TARGET": release["minimum_os"],
                "SANITIZER_MIN_OSX_VERSION": release["minimum_os"],
            }
        )
        for platform in ("IOS", "WATCHOS", "TVOS", "XROS"):
            options[f"COMPILER_RT_ENABLE_{platform}"] = "OFF"
    elif target["os"] == "windows":
        options.update(
            {
                "CMAKE_SYSTEM_NAME": "Windows",
                "CMAKE_SYSTEM_PROCESSOR": target["arch"],
                "CMAKE_POLICY_DEFAULT_CMP0091": "NEW",
                "CMAKE_MSVC_RUNTIME_LIBRARY": "MultiThreadedDLL",
            }
        )
    for language, compiler in (("C", cc), ("CXX", cxx), ("ASM", cc)):
        options[f"CMAKE_{language}_COMPILER"] = compiler.as_posix()
        options[f"CMAKE_{language}_COMPILER_TARGET"] = target["llvm_triple"]
        options[f"CMAKE_{language}_FLAGS"] = f"/clang:{flag}" if microsoft else shlex.join(flags)
    scratch = build / "scratch"
    scratch.mkdir(exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(scratch), TMP=str(scratch), TEMP=str(scratch))
    environment["PATH"] = str(cc.parent) + os.pathsep + environment.get("PATH", "")
    objects = build / "objects"
    configure_cmake(
        [
            str(tools[2]),
            "-G",
            "Ninja",
            "-S",
            str(source / "runtimes"),
            "-B",
            str(objects),
            *(f"-D{key}={value}" for key, value in options.items()),
        ],
        env=environment,
    )
    subprocess.run(
        [
            str(tools[2]),
            "--build",
            str(objects),
            "--target",
            "profile",
            "--parallel",
            str(args.jobs),
        ],
        env=environment,
        check=True,
    )
    archives = [
        path
        for path in objects.rglob("*clang_rt.profile*")
        if path.suffix in (".a", ".lib") and path.is_file()
    ]
    if len(archives) != 1:
        raise ValueError(f"Expected one target profile archive, found: {archives}")
    output.parent.mkdir(parents=True, exist_ok=True)
    staged = output.with_suffix(".new")
    shutil.copy2(archives[0], staged)
    staged.replace(output)
    save_record(record, inputs, objects, tools)
    print(f"Prepared {args.target} compiler-rt profile: {output}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--build-dir", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--cc", default="clang")
    parser.add_argument("--musl-include", type=Path)
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 8))
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    if (TARGETS[args.target]["env"] == "musl") != (args.musl_include is not None):
        parser.error("--musl-include is required exactly for a musl target")
    if args.musl_include:
        args.musl_include = args.musl_include.resolve()
    build = (args.build_dir or ROOT / "tmp/profile-runtime" / args.target).resolve()
    build.mkdir(parents=True, exist_ok=True)
    with build_lock(build / "build.lock"):
        prepare(args, build, args.output.resolve())


if __name__ == "__main__":
    main()
