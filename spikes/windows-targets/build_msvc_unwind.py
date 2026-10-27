#!/usr/bin/env python3
"""W0: build LLVM's SEH support with MSVC headers and /MD, without C++ EH."""

import argparse
import shutil
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--llvm", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True, help="LLVM 22.1.8 source root")
    parser.add_argument("--build-dir", type=Path, required=True)
    args = parser.parse_args()
    source = args.source.resolve() / "libunwind"
    build = args.build_dir.resolve()
    llvm = args.llvm.resolve()
    (build / "lib").mkdir(parents=True, exist_ok=True)
    # The upstream CMake entry rejects all MSVC targets as C++ ABI providers.
    # W0 builds its actual sources to test the distinct Scoop Level-I use case.
    # The production recipe must express this as a scoped upstream build patch.
    files = [
        "libunwind.cpp",
        "Unwind-EHABI.cpp",
        "Unwind-seh.cpp",
        "UnwindLevel1.c",
        "UnwindLevel1-gcc-ext.c",
        "Unwind-sjlj.c",
        "Unwind-wasm.c",
        "UnwindRegistersRestore.S",
        "UnwindRegistersSave.S",
    ]
    objects = []
    for filename in files:
        output = build / (filename + ".obj")
        flags = [
            "-target",
            "x86_64-pc-windows-msvc",
            "-O2",
            "-fms-runtime-lib=dll",
            "-D__SEH__",
            "-D_LIBUNWIND_IS_NATIVE_ONLY",
            "-D_LIBUNWIND_HIDE_SYMBOLS",
            "-D_CRT_SECURE_NO_WARNINGS",
            "-I",
            source / "include",
        ]
        if filename.endswith(".cpp"):
            flags += ["-fno-rtti", "-fno-exceptions"]
        elif filename.endswith(".c"):
            flags += ["-std=c11", "-fexceptions"]
        subprocess.run(
            [llvm / "bin/clang.exe", *flags, "-c", source / "src" / filename, "-o", output],
            check=True,
        )
        objects.append(output)
    subprocess.run(
        [llvm / "bin/llvm-lib.exe", "/out:" + str(build / "lib/unwind.lib"), *objects], check=True
    )
    shutil.copytree(source / "include", build / "include", dirs_exist_ok=True)
    print(f"Built MSVC /MD SEH experiment library: {build}")


if __name__ == "__main__":
    main()
