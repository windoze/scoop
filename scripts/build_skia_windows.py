"""Windows GN configuration for Skia's D3D12 and DirectWrite backends."""

import os
import shutil
import subprocess
from pathlib import Path

D3D_ALLOCATOR = "169895d529dfce00390a20e69c2f516066fe7a3b"
SPIRV_CROSS = "b8fcf307f1f347089e3c46eb4451d27f32ebc8d3"
ZLIB = "646b7f569718921d7d4b5b8e22572ff6c76f2596"
PACKAGES = (
    (
        f"D3D12MemoryAllocator-{D3D_ALLOCATOR}",
        f"D3D12MemoryAllocator-{D3D_ALLOCATOR}",
        "third_party/externals/d3d12allocator",
    ),
    (
        f"SPIRV-Cross-{SPIRV_CROSS}",
        f"SPIRV-Cross-{SPIRV_CROSS}",
        "third_party/externals/spirv-cross",
    ),
    (f"zlib-{ZLIB}", "", "third_party/externals/zlib"),
)
ARCHIVES = ("d3d12allocator", "spirv_cross", "zlib")


def configuration(args, release):
    microsoft = args.target == "x86_64-pc-windows-msvc-ucrt"
    compiler = args.cc or ("clang-cl" if microsoft else "clang")
    cc = shutil.which(compiler)
    if cc is None:
        raise ValueError(f"Skia preparation requires the target Clang: {compiler}")
    binary = Path(cc).parent
    ar = binary / "llvm-ar.exe"
    if not ar.is_file():
        raise ValueError(f"Skia preparation requires {ar}")
    options = {
        "target_os": "win",
        "target_cpu": "x64",
        "skia_use_metal": False,
        "skia_use_direct3d": True,
        "skia_use_system_zlib": False,
        "skia_use_xps": False,
        "is_mingw": not microsoft,
    }
    if microsoft:
        vc = os.environ.get("VCINSTALLDIR")
        sdk = os.environ.get("WINDOWSSDKDIR")
        if not vc or not sdk:
            raise ValueError(
                "Microsoft Skia preparation requires an x64 MSVC/Windows SDK environment"
            )
        options.update(
            clang_win=binary.parent.as_posix(),
            win_vc=Path(vc).as_posix(),
            win_sdk=Path(sdk).as_posix(),
            win_toolchain_version=release["msvc_toolset"],
            win_sdk_version=release["sdk"],
            extra_cflags=["/MD", "/clang:-march=" + release["cpu"]],
        )
        tools = [cc, str(ar)]
    else:
        cxx = shutil.which(args.cxx or str(binary / "clang++.exe"))
        if cxx is None:
            raise ValueError("LLVM-MinGW Skia preparation requires matching clang++")
        options.update(
            cc=Path(cc).as_posix(),
            cxx=Path(cxx).as_posix(),
            ar=ar.as_posix(),
            extra_cflags=["-march=" + release["cpu"], "-DWIN32_LEAN_AND_MEAN", "-DNOMINMAX"],
        )
        tools = [cc, cxx, str(ar)]
    return options, tools


def combine(args, objects, staged, names, ar):
    if args.target == "x86_64-pc-windows-msvc-ucrt":
        prefixed = {"png", "jpeg", "jpeg12", "jpeg16"}
        libraries = [objects / f"{'lib' if name in prefixed else ''}{name}.lib" for name in names]
    else:
        libraries = [objects / f"lib{name}.a" for name in names]
    # qL flattens each input archive; no nested archive members reach final linking.
    staged.unlink(missing_ok=True)
    subprocess.run([ar, "qcL", str(staged), *map(str, libraries)], check=True)
    subprocess.run([ar, "s", str(staged)], check=True)
