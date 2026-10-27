#!/usr/bin/env python3
"""W0: link the actual Rust text staticlib and Scoop SEH with each Windows CRT."""

import argparse
import os
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
RUST_TARGETS = {
    "msvc-ucrt": "x86_64-pc-windows-msvc",
    "mingw-ucrt": "x86_64-pc-windows-gnullvm",
    "mingw-msvcrt": "x86_64-pc-windows-gnu",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=RUST_TARGETS, required=True)
    parser.add_argument("--llvm", type=Path, required=True)
    parser.add_argument("--unwind", type=Path, required=True)
    parser.add_argument(
        "--vendor", type=Path, required=True, help="extracted text_backend crates directory"
    )
    parser.add_argument("--cargo-target-dir", type=Path, required=True)
    parser.add_argument("--build-dir", type=Path, required=True)
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("run this probe on Windows")
    build = args.build_dir.resolve()
    build.mkdir(parents=True, exist_ok=True)
    llvm = args.llvm.resolve()
    unwind = args.unwind.resolve()
    source = ROOT / "sysroot/native/text_backend"
    rust_target = RUST_TARGETS[args.target]
    msvc = args.target == "msvc-ucrt"
    cargo_target = args.cargo_target_dir.resolve()
    command = [
        "cargo",
        "--config",
        'source.crates-io.replace-with="scoop-vendor"',
        "--config",
        f'source.scoop-vendor.directory="{args.vendor.resolve().as_posix()}"',
        "rustc",
        "--manifest-path",
        source / "Cargo.toml",
        "--locked",
        "--offline",
        "--release",
        "--target",
        rust_target,
        "--target-dir",
        cargo_target,
        "--",
        "--print=native-static-libs",
    ]
    result = subprocess.run(command, text=True, capture_output=True)
    output = result.stdout + result.stderr
    (build / "rust.txt").write_text(output, encoding="utf-8")
    if result.returncode:
        print(output, flush=True)
        result.check_returncode()
    native = re.search(r"native-static-libs: (.+)", output)
    if native is None:
        raise RuntimeError("Rust did not report the staticlib's native dependencies")
    dependencies = native.group(1).split()
    if msvc:
        dependencies = [part for item in dependencies for part in ("-Xlinker", item)]
    cc = llvm / "bin/clang.exe"
    flags = [
        "-std=c11",
        "-O2",
        "-D__SEH__",
        "-I",
        unwind / "include",
        "-I",
        source / "include",
        "-I",
        ROOT / "runtime/src",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-Wno-override-module",
    ]
    if msvc:
        flags += ["--target=x86_64-pc-windows-msvc", "-fms-runtime-lib=dll", "-fuse-ld=lld"]
    sources = [
        HERE / "text.c",
        HERE / "eh.c",
        ROOT / "runtime/src/eh/lsda.c",
        ROOT / "runtime/src/eh_personality.c",
    ]
    subprocess.run([cc, *flags, "-fsyntax-only", *sources], check=True)
    library = (
        cargo_target
        / rust_target
        / "release"
        / ("scoop_text_backend.lib" if msvc else "libscoop_text_backend.a")
    )
    unwind_library = unwind / "lib" / ("unwind.lib" if msvc else "libunwind.a")
    executable = build / "text.exe"
    subprocess.run(
        [
            cc,
            *flags,
            *sources,
            HERE / "eh.ll",
            library,
            unwind_library,
            *dependencies,
            "-o",
            executable,
        ],
        check=True,
    )
    subprocess.run([executable], check=True)
    imports = subprocess.check_output(
        [llvm / "bin/llvm-readobj.exe", "--coff-imports", executable], text=True
    )
    (build / "imports.txt").write_text(imports, encoding="utf-8")
    folded = imports.lower()
    old_crt = "name: msvcrt.dll" in folded
    ucrt = "ucrtbase.dll" in folded or "api-ms-win-crt-" in folded
    if old_crt != (args.target == "mingw-msvcrt") or ucrt == old_crt or "libgcc" in folded:
        raise RuntimeError(f"unexpected CRT/unwinder imports for {args.target}")
    # Qualification only: inspect the real archive for a second Level-I provider.
    symbols = subprocess.check_output(
        [llvm / "bin/llvm-nm.exe", "--defined-only", library], text=True
    )
    if re.search(r"\b[TW] _Unwind_", symbols):
        raise RuntimeError("text staticlib contains another Level-I unwind implementation")
    print(f"{args.target}: Rust text staticlib and native dependency closure passed")


if __name__ == "__main__":
    main()
