#!/usr/bin/env python3
"""Build Python-enabled LLVM 22 LLDB against an installed LLVM/Clang SDK."""

import argparse
import os
import platform
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

from build_llvm import HOSTS, VERSION
from build_support import RELEASE_CONFIG, ROOT, metadata, reusable, save_record, source_metadata

PATCH = ROOT / "scripts/patches/lldb-scoop-source-names.patch"
SOURCE_PARTS = ("lldb", "cmake", "llvm/cmake", "llvm/utils", "third-party")


def configure(source, build, prefix, llvm, python, host, release):
    target = release["targets"][host]
    options = {
        "CMAKE_BUILD_TYPE": "Release",
        "CMAKE_INSTALL_PREFIX": str(prefix),
        "CMAKE_INSTALL_RPATH": (
            ("@loader_path/../lib" if host.endswith("darwin") else "$ORIGIN/../lib")
            + ";"
            + str(llvm / "lib")
        ),
        "LLVM_DIR": str(llvm / "lib/cmake/llvm"),
        "Clang_DIR": str(llvm / "lib/cmake/clang"),
        "LLVM_LINK_LLVM_DYLIB": "ON",
        "CLANG_LINK_CLANG_DYLIB": "ON",
        "LLVM_APPEND_VC_REV": "OFF",
        "LLDB_INCLUDE_TESTS": "OFF",
        "LLDB_ENABLE_PYTHON": "ON",
        "LLDB_EMBED_PYTHON_HOME": "OFF",
        "Python3_EXECUTABLE": python,
        "LLDB_ENABLE_LUA": "OFF",
        "LLDB_ENABLE_LIBEDIT": "OFF",
        "LLDB_ENABLE_LIBXML2": "OFF",
        "LLDB_BUILD_FRAMEWORK": "OFF",
        "LLDB_SKIP_DSYM": "ON",
        "CMAKE_C_FLAGS": "-mcpu=generic" if host.endswith("darwin") else "-march=" + target["cpu"],
        "CMAKE_CXX_FLAGS": "-mcpu=generic"
        if host.endswith("darwin")
        else "-march=" + target["cpu"],
    }
    if host.endswith("darwin"):
        options["CMAKE_OSX_DEPLOYMENT_TARGET"] = target["minimum_os"]
        options["CMAKE_OSX_ARCHITECTURES"] = "arm64"
        options["LLDB_CODESIGN_IDENTITY"] = "-"
    return [
        "cmake",
        "-G",
        "Ninja",
        "-S",
        str(source / "lldb"),
        "-B",
        str(build / "objects"),
        *(f"-D{key}={value}" for key, value in options.items()),
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--llvm-prefix", required=True, type=Path)
    parser.add_argument("--build-dir", required=True, type=Path)
    parser.add_argument("--prefix", required=True, type=Path)
    parser.add_argument("--python", default=sys.executable)
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    host = HOSTS.get((platform.system(), platform.machine()))
    if host is None or args.jobs < 1:
        parser.error("a supported host and positive --jobs are required")
    source, llvm, build, prefix = (
        path.resolve() for path in (args.source, args.llvm_prefix, args.build_dir, args.prefix)
    )
    python = shutil.which(args.python)
    swig = shutil.which("swig")
    if not python or not swig or not shutil.which("ninja"):
        parser.error("Python development files, SWIG 4 and Ninja are required")
    server = "debugserver" if host.endswith("darwin") else "lldb-server"
    outputs = [prefix / "bin" / name for name in ("lldb", "lldb-dap", "lldb-argdumper", server)]
    outputs += list((prefix / "lib").glob("python*/site-packages/lldb/__init__.py"))
    outputs += list((prefix / "lib").glob("python*/dist-packages/lldb/__init__.py"))
    inputs = {
        "source": source_metadata(source, SOURCE_PARTS),
        "tools": [
            metadata(path)
            for path in (
                python,
                swig,
                __file__,
                PATCH,
                RELEASE_CONFIG,
                ROOT / "scripts/build_support.py",
                llvm / "bin/llvm-config",
            )
        ],
        "sdk": source_metadata(llvm, ("include/llvm", "include/clang", "lib/cmake")),
        "host": host,
        "prefix": str(prefix),
    }
    record = build / "inputs.json"
    if len(outputs) > 4 and reusable(record, inputs, outputs):
        print(f"Reusing LLDB {VERSION}: {prefix}")
        return
    actual = subprocess.check_output(
        [str(llvm / "bin/llvm-config"), "--version"], text=True
    ).strip()
    if actual != VERSION:
        parser.error(f"LLDB requires LLVM {VERSION}, found {actual}")
    copied = build / "source"
    if copied.exists():
        shutil.rmtree(copied)
    for component in SOURCE_PARTS:
        shutil.copytree(source / component, copied / component)
    subprocess.run(["patch", "-p1", "-i", str(PATCH)], cwd=copied, check=True)
    release = tomllib.loads(RELEASE_CONFIG.read_text())
    subprocess.run(configure(copied, build, prefix, llvm, python, host, release), check=True)
    subprocess.run(
        [
            "cmake",
            "--build",
            str(build / "objects"),
            "--parallel",
            str(args.jobs),
            "--target",
            "install-lldb",
            "install-lldb-dap",
            "install-liblldb",
            "install-lldb-python-scripts",
            "install-lldb-argdumper",
            "install-" + server,
        ],
        check=True,
    )
    save_record(record, inputs, build / "objects")
    print(f"Installed LLDB {VERSION}: {prefix}")


if __name__ == "__main__":
    main()
