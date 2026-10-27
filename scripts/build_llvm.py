#!/usr/bin/env python3
"""Build the LLVM 22.1.8 shared backend and native tools for Scoop hosts."""

import argparse
import os
import platform
import shutil
import subprocess
import tomllib
from pathlib import Path

from build_support import RELEASE_CONFIG, ROOT, metadata, reusable, save_record, source_metadata

VERSION = "22.1.8"
STATEPOINT_PATCH = (
    ROOT / "sysroot/vendor/llvm-project/22.1.8/patches/llvm-windows-statepoint-return-pc.patch"
)
HOSTS = {
    ("Darwin", "arm64"): "aarch64-apple-darwin",
    ("Linux", "x86_64"): "x86_64-unknown-linux-gnu",
    ("Windows", "AMD64"): "x86_64-pc-windows-gnullvm",
}

TOOLS = (
    "llvm-config",
    "clang",
    "clang++",
    "clang-cl",
    "lld",
    "lld-link",
    "ld.lld",
    "llvm-ar",
    "llvm-ranlib",
    "llvm-readobj",
    "llvm-rc",
    "llvm-cvtres",
    "llvm-nm",
    "llvm-objcopy",
    "llvm-strip",
    "llvm-symbolizer",
)
WINDOWS_RUNTIME_DLLS = ("libc++.dll", "libunwind.dll")


def patch_windows_statepoint(source):
    command = ["git", "-C", str(source), "apply", "--check", str(STATEPOINT_PATCH)]
    if (
        subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode
        == 0
    ):
        subprocess.run(["git", "-C", str(source), "apply", str(STATEPOINT_PATCH)], check=True)
        return source / "llvm/lib/Target/X86/X86MCInstLower.cpp"
    subprocess.run(
        ["git", "-C", str(source), "apply", "--reverse", "--check", str(STATEPOINT_PATCH)],
        check=True,
    )
    return None


def configure(source, build, prefix, cc, cxx, host, release):
    windows = host.endswith("windows-gnullvm")
    target = release["targets"]["x86_64-w64-windows-llvm-mingw-ucrt" if windows else host]
    cpu = "-mcpu=generic" if host.endswith("darwin") else "-march=" + target["cpu"]
    options = {
        "CMAKE_BUILD_TYPE": "Release",
        "CMAKE_INSTALL_PREFIX": str(prefix),
        "CMAKE_C_COMPILER": cc,
        "CMAKE_CXX_COMPILER": cxx,
        "CMAKE_C_FLAGS": cpu,
        "CMAKE_CXX_FLAGS": cpu,
        "LLVM_TARGETS_TO_BUILD": "AArch64;X86",
        "LLVM_BUILD_LLVM_DYLIB": "ON",
        "LLVM_LINK_LLVM_DYLIB": "ON",
        "CLANG_LINK_CLANG_DYLIB": "ON",
        "BUILD_SHARED_LIBS": "OFF",
        "LLVM_DYLIB_COMPONENTS": "all",
        "LLVM_ENABLE_PROJECTS": "clang;lld",
        "LLVM_ENABLE_RUNTIMES": "",
        "LLVM_DISTRIBUTION_COMPONENTS": ";".join(
            (
                "LLVM",
                "clang-cpp",
                "llvm-headers",
                "cmake-exports",
                "clang-resource-headers",
                *(
                    tool
                    for tool in TOOLS
                    if tool not in ("clang++", "clang-cl", "lld-link", "ld.lld")
                ),
            )
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
    if windows:
        options.update(
            {
                "CMAKE_C_COMPILER_TARGET": "x86_64-w64-windows-gnu",
                "CMAKE_CXX_COMPILER_TARGET": "x86_64-w64-windows-gnu",
                "CMAKE_AR": str(Path(cc).parent / "llvm-ar.exe"),
                "CMAKE_RANLIB": str(Path(cc).parent / "llvm-ranlib.exe"),
                "LLVM_USE_LINKER": "lld",
            }
        )
    else:
        options["CMAKE_INSTALL_RPATH"] = (
            "@loader_path/../lib" if host.endswith("darwin") else "$ORIGIN/../lib"
        )
    if host.endswith("darwin"):
        options["CMAKE_OSX_DEPLOYMENT_TARGET"] = target["minimum_os"]
        options["CMAKE_OSX_ARCHITECTURES"] = "arm64"
    return [
        "cmake",
        "-G",
        "Ninja" if windows else "Unix Makefiles",
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
        parser.error("build host must be Darwin/AArch64, Linux/GNU amd64 or Windows/amd64")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    cc, cxx = shutil.which(args.cc), shutil.which(args.cxx)
    if not cc or not cxx:
        parser.error("--cc and --cxx must name installed host compilers")
    source = args.source.resolve()
    build = (args.build_dir or ROOT / "tmp/llvm" / host).resolve()
    prefix = (args.prefix or build / "installed").resolve()
    windows = host.endswith("windows-gnullvm")
    suffix = ".exe" if windows else ""
    ninja = shutil.which("ninja") if windows else None
    if windows and ninja is None:
        parser.error("Windows LLVM builds require Ninja in PATH")
    companions = [Path(cxx).parent / name for name in WINDOWS_RUNTIME_DLLS] if windows else []
    inputs = {
        "source": source_metadata(source, ("llvm", "clang", "lld", "cmake", "third-party")),
        "tools": [
            metadata(path)
            for path in (
                cc,
                cxx,
                __file__,
                ROOT / "scripts/build_support.py",
                RELEASE_CONFIG,
                STATEPOINT_PATCH,
                *companions,
                *([ninja] if windows else []),
            )
        ],
        "host": host,
        "prefix": str(prefix),
    }
    outputs = [prefix / "bin" / (name + suffix) for name in TOOLS]
    outputs += [prefix / "include/llvm-c/Core.h"]
    outputs += [prefix / "lib/clang/22/include/stddef.h"]
    outputs += [prefix / "bin" / path.name for path in companions]
    if windows:
        outputs += [prefix / "lib/libLLVM-22.dll.a"]
    llvm_libraries = list(
        (prefix / ("bin" if windows else "lib")).glob(
            "libLLVM*.dll"
            if windows
            else "libLLVM*.dylib"
            if host.endswith("darwin")
            else "libLLVM*.so*"
        )
    )
    clang_libraries = list(
        (prefix / ("bin" if windows else "lib")).glob(
            "libclang-cpp.dll"
            if windows
            else "libclang-cpp*.dylib"
            if host.endswith("darwin")
            else "libclang-cpp*.so*"
        )
    )
    outputs += llvm_libraries + clang_libraries
    record = build / "inputs.json"
    if llvm_libraries and clang_libraries and reusable(record, inputs, outputs):
        print(f"Reusing LLVM {VERSION}: {prefix}")
        return
    patched = patch_windows_statepoint(source)
    if patched is not None:
        # Applying the source fix changed one input after the update decision.
        inputs["source"] = [
            metadata(patched) if row[0] == str(patched) else row for row in inputs["source"]
        ]
    release = tomllib.loads(RELEASE_CONFIG.read_text())
    if release["llvm"] != VERSION:
        parser.error(f"this recipe requires LLVM {VERSION}")
    scratch = build / "scratch"
    scratch.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(scratch))
    if windows:
        environment["PATH"] = str(Path(cc).parent) + os.pathsep + environment["PATH"]
    configuration = configure(source, build, prefix, cc, cxx, host, release)
    if windows:
        configuration.append(f"-DCMAKE_MAKE_PROGRAM={ninja}")
    subprocess.run(configuration, check=True, env=environment)
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
    for path in companions:
        shutil.copy2(path, prefix / "bin" / path.name)
    actual = subprocess.check_output(
        [str(prefix / "bin" / ("llvm-config" + suffix)), "--version"], text=True
    ).strip()
    if actual != VERSION:
        raise ValueError(f"expected LLVM {VERSION}, built {actual}")
    subprocess.run(
        [str(prefix / "bin" / ("clang" + suffix)), "--version"], check=True, env=environment
    )
    save_record(record, inputs, build / "objects")
    print(f"Installed LLVM {VERSION}: {prefix}")


if __name__ == "__main__":
    main()
