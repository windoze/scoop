#!/usr/bin/env python3
"""W0: consume real native C++ static and DLL libraries alongside Scoop SEH."""

import argparse
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--target", choices=("msvc-ucrt", "mingw-ucrt", "mingw-msvcrt"), required=True
    )
    parser.add_argument("--llvm", type=Path, required=True)
    parser.add_argument("--unwind", type=Path, required=True)
    parser.add_argument("--build-dir", type=Path, required=True)
    parser.add_argument("--toolset", type=Path, help="MSVC toolset version directory")
    parser.add_argument("--sdk", type=Path, help="Windows Kits/10 directory")
    parser.add_argument("--sdk-version", help="SDK include/lib version")
    args = parser.parse_args()
    msvc = args.target == "msvc-ucrt"
    if os.name != "nt":
        parser.error("run the W0 native probes on Windows")
    if msvc and not all((args.toolset, args.sdk, args.sdk_version)):
        parser.error("MSVC needs --toolset, --sdk and --sdk-version")
    build = args.build_dir.resolve()
    build.mkdir(parents=True, exist_ok=True)
    llvm = args.llvm.resolve()
    unwind = args.unwind.resolve()
    env = dict(os.environ)
    if msvc:
        toolset = args.toolset.resolve()
        sdk = args.sdk.resolve()
        includes = [toolset / "include"] + [
            sdk / "Include" / args.sdk_version / part for part in ("ucrt", "shared", "um", "winrt")
        ]
        libs = [toolset / "lib/x64"] + [
            sdk / "Lib" / args.sdk_version / part / "x64" for part in ("ucrt", "um")
        ]
        env["INCLUDE"] = os.pathsep.join(str(path) for path in includes)
        env["LIB"] = os.pathsep.join(str(path) for path in libs)
        env["PATH"] = str(toolset / "bin/Hostx64/x64") + os.pathsep + env["PATH"]
    else:
        env["PATH"] = str(llvm / "bin") + os.pathsep + env["PATH"]

    def run(command):
        with (build / "commands.txt").open("a", encoding="utf-8") as log:
            log.write(subprocess.list2cmdline([str(arg) for arg in command]) + "\n")
            result = subprocess.run(command, cwd=build, env=env, capture_output=True, text=True)
            log.write(result.stdout + result.stderr)
        if result.returncode:
            print(result.stdout + result.stderr, flush=True)
            result.check_returncode()
        return result.stdout

    cc = llvm / "bin/clang.exe"
    cflags = [
        "-O2",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-Wno-override-module",
        "-D__SEH__",
        "-I",
        unwind / "include",
        "-I",
        ROOT / "runtime/src",
    ]
    if msvc:
        cflags += ["--target=x86_64-pc-windows-msvc", "-fms-runtime-lib=dll"]
        provider = toolset / "bin/Hostx64/x64/cl.exe"
        cxxflags = ["/nologo", "/std:c++20", "/EHsc", "/MD", "/O2", "/W4", "/WX"]
        run(
            [
                provider,
                *cxxflags,
                "/c",
                "/DPROBE_NAMESPACE=static_library",
                HERE / "native.cpp",
                "/Fo:static.obj",
            ]
        )
        run([llvm / "bin/llvm-lib.exe", "/out:static.lib", "static.obj"])
        run(
            [
                provider,
                *cxxflags,
                "/LD",
                "/DPROBE_NAMESPACE=dynamic_library",
                "/DPROBE_DLL_EXPORT",
                HERE / "native.cpp",
                "/Fo:dynamic.obj",
                "/Fe:dynamic.dll",
                "/link",
                "/implib:dynamic.lib",
            ]
        )
        libraries = ["static.lib", "dynamic.lib", unwind / "lib/unwind.lib"]
        wrappers = [("cl", provider), ("clang-cl", llvm / "bin/clang-cl.exe")]
    else:
        provider = llvm / "bin/clang++.exe"
        cxxflags = ["-std=c++20", "-O2", "-Wall", "-Wextra", "-Werror", "-stdlib=libc++"]
        run(
            [
                provider,
                *cxxflags,
                "-c",
                "-DPROBE_NAMESPACE=static_library",
                HERE / "native.cpp",
                "-o",
                "static.obj",
            ]
        )
        run([llvm / "bin/llvm-ar.exe", "rcs", "libstatic.a", "static.obj"])
        run(
            [
                provider,
                *cxxflags,
                "-shared",
                "-static",
                "-DPROBE_NAMESPACE=dynamic_library",
                "-DPROBE_DLL_EXPORT",
                HERE / "native.cpp",
                "-o",
                "dynamic.dll",
                "-Wl,--out-implib,libdynamic.dll.a",
            ]
        )
        libraries = ["libstatic.a", "libdynamic.dll.a", unwind / "lib/libunwind.a"]
        wrappers = [("clang++", provider)]

    sources = [
        HERE / "native_main.c",
        HERE / "eh.c",
        HERE / "eh.ll",
        ROOT / "runtime/src/eh/lsda.c",
        ROOT / "runtime/src/eh_personality.c",
    ]
    objects = []
    for index, source in enumerate(sources):
        output = f"probe-{index}.obj"
        flags = cflags if source.suffix != ".ll" else ["-O2", "-Wno-override-module"]
        run([cc, *flags, "-c", source, "-o", output])
        objects.append(output)
    for name, wrapper in wrappers:
        executable = build / (name + ".exe")
        if msvc:
            run([wrapper, *cxxflags, "/c", HERE / "native_wrapper.cpp", "/Fowrapper.obj"])
            linker = (
                llvm / "bin/lld-link.exe"
                if name == "clang-cl"
                else toolset / "bin/Hostx64/x64/link.exe"
            )
            run([linker, "/nologo", "/out:" + str(executable), "wrapper.obj", *objects, *libraries])
        else:
            run(
                [
                    wrapper,
                    *cxxflags,
                    "-static",
                    HERE / "native_wrapper.cpp",
                    *objects,
                    *libraries,
                    "-o",
                    executable,
                ]
            )
        actual = run([executable])
        expected = (
            "SEH catch/cleanup/resume passed\nnative C/C++ and Scoop SEH passed\n"
            "native destructor\nnative destructor\n"
        )
        if actual != expected:
            raise RuntimeError(f"unexpected native output: {actual!r}")
        imports = run([llvm / "bin/llvm-readobj.exe", "--coff-imports", executable])
        (build / (name + "-imports.txt")).write_text(imports, encoding="utf-8")
        print(
            f"{args.target}/{name}: static+DLL C++ ABI, RTTI, native EH, Scoop SEH, TLS and locale passed"
        )


if __name__ == "__main__":
    main()
