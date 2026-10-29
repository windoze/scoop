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


def copy_sources(source, copied):
    previous = (
        {
            Path(path).relative_to(copied): (mtime, size)
            for path, mtime, size in source_metadata(copied, SOURCE_PARTS)
        }
        if copied.exists()
        else {}
    )

    def copy_updated(original, destination):
        result = shutil.copy2(original, destination)
        info = Path(destination).stat()
        old = previous.get(Path(destination).relative_to(copied))
        if old is not None and old != (
            info.st_mtime_ns,
            info.st_size,
        ):
            # Removing a patch restores the archive's older timestamp. Ninja
            # must still rebuild that changed source, without touching others.
            os.utime(destination, None)
        return result

    if copied.exists():
        shutil.rmtree(copied)
    for component in SOURCE_PARTS:
        shutil.copytree(source / component, copied / component, copy_function=copy_updated)


def configure(source, build, prefix, llvm, python, host, release, cc, cxx):
    windows = "windows" in host
    target = release["targets"]["x86_64-w64-windows-llvm-mingw-ucrt" if windows else host]
    options = {
        "CMAKE_BUILD_TYPE": "Release",
        "CMAKE_INSTALL_PREFIX": str(prefix),
        "CMAKE_C_COMPILER": cc,
        "CMAKE_CXX_COMPILER": cxx,
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
    if windows:
        options.update(
            {
                "CMAKE_C_COMPILER_TARGET": "x86_64-w64-windows-gnu",
                "CMAKE_CXX_COMPILER_TARGET": "x86_64-w64-windows-gnu",
                "CMAKE_AR": str(Path(cc).parent / "llvm-ar.exe"),
                "CMAKE_RANLIB": str(Path(cc).parent / "llvm-ranlib.exe"),
                "LLVM_USE_LINKER": "lld",
                "LLDB_ENABLE_PYTHON_LIMITED_API": "ON",
            }
        )
    else:
        options["CMAKE_INSTALL_RPATH"] = (
            ("@loader_path/../lib" if host.endswith("darwin") else "$ORIGIN/../lib")
            + ";"
            + str(llvm / "lib")
        )
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
    parser.add_argument("--cc", default="cc")
    parser.add_argument("--cxx", default="c++")
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
    cc, cxx, ninja = (shutil.which(name) for name in (args.cc, args.cxx, "ninja"))
    if not all((python, swig, cc, cxx, ninja)):
        parser.error("C/C++ compilers, Python development files, SWIG 4 and Ninja are required")
    suffix = ".exe" if "windows" in host else ""
    llvm_config = llvm / "bin" / ("llvm-config" + suffix)
    server = "debugserver" if host.endswith("darwin") else "lldb-server"
    outputs = [
        prefix / "bin" / (name + suffix) for name in ("lldb", "lldb-dap", "lldb-argdumper", server)
    ]
    # Debian's Python install scheme uses local/lib inside an explicit prefix.
    outputs += list(prefix.glob("**/lldb/__init__.py"))
    inputs = {
        "source": source_metadata(source, SOURCE_PARTS),
        "tools": [
            metadata(path)
            for path in (
                python,
                swig,
                cc,
                cxx,
                ninja,
                __file__,
                PATCH,
                RELEASE_CONFIG,
                ROOT / "scripts/build_support.py",
                llvm_config,
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
    actual = subprocess.check_output([str(llvm_config), "--version"], text=True).strip()
    if actual != VERSION:
        parser.error(f"LLDB requires LLVM {VERSION}, found {actual}")
    copied = build / "source"
    if not copied.resolve().is_relative_to(build) or source.is_relative_to(copied.resolve()):
        parser.error(
            "the build's copied source must remain inside --build-dir and separate from --source"
        )
    copy_sources(source, copied)
    subprocess.run(["git", "apply", "--no-index", str(PATCH)], cwd=copied, check=True)
    release = tomllib.loads(RELEASE_CONFIG.read_text())
    configuration = configure(copied, build, prefix, llvm, python, host, release, cc, cxx)
    configuration.append(f"-DCMAKE_MAKE_PROGRAM={ninja}")
    subprocess.run(configuration, check=True)
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
