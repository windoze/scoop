"""Select Windows host compilers and install their native development resources."""

import os
import shutil
from pathlib import Path

from build_llvm import TOOLS
from target_catalog import native_configuration


def arguments(parser):
    parser.add_argument(
        "--mingw-msvcrt", type=Path, help="LLVM-MinGW MSVCRT prefix for Windows packaging"
    )
    parser.add_argument(
        "--mingw-ucrt", type=Path, help="LLVM-MinGW UCRT prefix for Windows packaging"
    )
    parser.add_argument(
        "--windows-perl", default="perl", help="Native Windows Perl for MSVC OpenSSL"
    )
    parser.add_argument("--msys-perl", default="perl", help="MSYS Perl for MinGW OpenSSL")
    parser.add_argument("--msys-make", default="make", help="GNU Make for MinGW OpenSSL")
    parser.add_argument("--msys-shell", default="sh", help="MSYS shell for MinGW OpenSSL")


def mingw_prefix(args, target):
    if native_configuration(target)["native_environment"] == "LlvmMingwMsvcrt":
        return args.mingw_msvcrt
    if native_configuration(target)["native_environment"] == "LlvmMingwUcrt":
        return args.mingw_ucrt
    return None


def host_environment(args, environment):
    args.mingw_msvcrt = args.mingw_msvcrt.resolve(strict=True)
    args.mingw_ucrt = args.mingw_ucrt.resolve(strict=True)
    binary = args.mingw_ucrt / "bin"
    environment["CARGO_BUILD_TARGET"] = args.host
    environment["CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER"] = str(binary / "clang.exe")
    environment["CC_x86_64_pc_windows_gnullvm"] = str(binary / "clang.exe")
    environment["CXX_x86_64_pc_windows_gnullvm"] = str(binary / "clang++.exe")
    environment["AR_x86_64_pc_windows_gnullvm"] = str(binary / "llvm-ar.exe")
    environment["PATH"] = str(binary) + os.pathsep + environment.get("PATH", "")


def compiler(args, target):
    prefix = mingw_prefix(args, target)
    if prefix is not None:
        return prefix / "bin/clang.exe"
    return args.llvm_prefix / "bin/clang-cl.exe"


def openssl_arguments(args, target):
    if mingw_prefix(args, target) is None:
        return ["--perl", args.windows_perl]
    return ["--perl", args.msys_perl, "--make", args.msys_make, "--shell", args.msys_shell]


def install(root, targets, args):
    private = root / "libexec/llvm"
    (private / "bin").mkdir(parents=True)
    for name in TOOLS:
        shutil.copy2(args.llvm_prefix / "bin" / (name + ".exe"), private / "bin")
    shutil.copytree(args.llvm_prefix / "lib/clang", private / "lib/clang")
    for target in targets:
        prefix = mingw_prefix(args, target)
        if prefix is None:
            continue
        shutil.copytree(
            prefix,
            root / "lib" / target / "toolchain",
            ignore=shutil.ignore_patterns(
                "aarch64-w64-mingw32*",
                "arm64ec-w64-mingw32*",
                "armv7-w64-mingw32*",
                "i686-w64-mingw32*",
            ),
        )
