#!/usr/bin/env python3
"""Build the LLVM/Clang 22.1.8 SDK and tools used by Scoop and LLDB."""

import argparse
import os
import platform
import shutil
import subprocess
import tomllib
from pathlib import Path

from build_support import RELEASE_CONFIG, ROOT, metadata, reusable, save_record, source_metadata

VERSION = "22.1.8"
HOSTS = {
    ("Darwin", "arm64"): "aarch64-apple-darwin",
    ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
}


def configure(source, build, prefix, cc, cxx, host, release):
    target = release["targets"][host]
    cpu = "-mcpu=generic" if host.endswith("darwin") else "-march=" + target["cpu"]
    options = {
        "CMAKE_BUILD_TYPE": "Release",
        "CMAKE_INSTALL_PREFIX": str(prefix),
        "CMAKE_C_COMPILER": cc,
        "CMAKE_CXX_COMPILER": cxx,
        "CMAKE_C_FLAGS": cpu,
        "CMAKE_CXX_FLAGS": cpu,
        "CMAKE_INSTALL_RPATH": "@loader_path/../lib"
        if host.endswith("darwin")
        else "$ORIGIN/../lib",
        "LLVM_TARGETS_TO_BUILD": "AArch64;X86",
        "LLVM_BUILD_LLVM_DYLIB": "ON",
        "LLVM_LINK_LLVM_DYLIB": "ON",
        "BUILD_SHARED_LIBS": "OFF",
        "LLVM_DYLIB_COMPONENTS": "all",
        "LLVM_ENABLE_PROJECTS": "clang",
        "CLANG_LINK_CLANG_DYLIB": "ON",
        "LLVM_ENABLE_RUNTIMES": "",
        "LLVM_DISTRIBUTION_COMPONENTS": (
            "LLVM;clang-cpp;clang-headers;clang-cmake-exports;clang-resource-headers;"
            "llvm-config;llvm-headers;cmake-exports;dsymutil;llvm-objcopy;llvm-strip;llvm-symbolizer;llvm-nm;llvm-readobj"
        ),
        "LLVM_INCLUDE_TESTS": "OFF",
        "LLVM_INCLUDE_DOCS": "OFF",
        "LLVM_INCLUDE_EXAMPLES": "OFF",
        "LLVM_INCLUDE_BENCHMARKS": "OFF",
        "LLVM_ENABLE_BINDINGS": "OFF",
        "LLVM_ENABLE_PLUGINS": "OFF",
        "LLVM_ENABLE_ZLIB": "OFF",
        "LLVM_ENABLE_ZSTD": "OFF",
        "LLVM_ENABLE_LIBXML2": "OFF",
        "LLVM_ENABLE_LIBEDIT": "OFF",
        "LLVM_ENABLE_LIBPFM": "OFF",
        "LLVM_ENABLE_FFI": "OFF",
        "LLVM_ENABLE_CURL": "OFF",
    }
    if host.endswith("darwin"):
        options["CMAKE_OSX_DEPLOYMENT_TARGET"] = target["minimum_os"]
        options["CMAKE_OSX_ARCHITECTURES"] = "arm64"
    return [
        "cmake",
        "-G",
        "Unix Makefiles",
        "-S",
        str(source / "llvm"),
        "-B",
        str(build / "objects"),
        *(f"-D{key}={value}" for key, value in options.items()),
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        type=Path,
        default=ROOT / f"sysroot/vendor/llvm-project/{VERSION}/llvm-project-{VERSION}.src",
    )
    parser.add_argument("--build-dir", type=Path)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--cc", default="cc")
    parser.add_argument("--cxx", default="c++")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    host = HOSTS.get((platform.system(), platform.machine()))
    if host is None:
        parser.error("build host must be Darwin/AArch64 or Linux/GNU amd64")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    cc, cxx = shutil.which(args.cc), shutil.which(args.cxx)
    if not cc or not cxx:
        parser.error("--cc and --cxx must name installed host compilers")
    source = args.source.resolve()
    build = (args.build_dir or ROOT / "tmp/llvm" / host).resolve()
    prefix = (args.prefix or build / "installed").resolve()
    inputs = {
        "source": source_metadata(source, ("llvm", "clang", "cmake", "third-party")),
        "tools": [
            metadata(path)
            for path in (cc, cxx, __file__, ROOT / "scripts/build_support.py", RELEASE_CONFIG)
        ],
        "host": host,
        "prefix": str(prefix),
    }
    outputs = [
        prefix / "bin" / name
        for name in (
            "llvm-config",
            "dsymutil",
            "llvm-objcopy",
            "llvm-strip",
            "llvm-symbolizer",
            "llvm-nm",
            "llvm-readobj",
        )
    ]
    outputs += [prefix / "include/llvm-c/Core.h", prefix / "lib/cmake/clang/ClangConfig.cmake"]
    outputs += list(
        (prefix / "lib").glob("libLLVM*.dylib" if host.endswith("darwin") else "libLLVM*.so*")
    )
    record = build / "inputs.json"
    if len(outputs) > 9 and reusable(record, inputs, outputs):
        print(f"Reusing LLVM {VERSION}: {prefix}")
        return
    release = tomllib.loads(RELEASE_CONFIG.read_text())
    if release["llvm"] != VERSION:
        parser.error(f"this recipe requires LLVM {VERSION}")
    scratch = build / "scratch"
    scratch.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(scratch))
    subprocess.run(
        configure(source, build, prefix, cc, cxx, host, release), check=True, env=environment
    )
    subprocess.run(
        [
            "cmake",
            "--build",
            str(build / "objects"),
            "--target",
            "install-distribution",
            "--parallel",
            str(args.jobs),
        ],
        check=True,
        env=environment,
    )
    actual = subprocess.check_output(
        [str(prefix / "bin/llvm-config"), "--version"], text=True
    ).strip()
    if actual != VERSION:
        raise ValueError(f"expected LLVM {VERSION}, built {actual}")
    save_record(record, inputs, build / "objects")
    print(f"Installed LLVM {VERSION}: {prefix}")


if __name__ == "__main__":
    main()
