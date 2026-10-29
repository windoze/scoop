"""Clang/LLVM companions for the selected Windows native environment."""

import os
import shutil
import tomllib
from pathlib import Path

from .model import EnvironmentError


def companions(target, args, sysroot, needed):
    microsoft = target == "x86_64-pc-windows-msvc-ucrt"
    root = (args.native_sysroot or sysroot / "lib" / target / "toolchain").resolve()
    llvm = os.environ.get("LLVM_SYS_221_PREFIX")
    tool_root = root
    if microsoft:
        tool_root = Path(llvm) if llvm else None
    result = {"cc_args": []}
    for name, filename in [("cc", "clang.exe"), ("ar", "llvm-ar.exe")]:
        if name not in needed:
            continue
        configured = getattr(args, name)
        program = str(configured or (tool_root / "bin" / filename if tool_root else filename))
        found = shutil.which(program)
        if found is None:
            raise EnvironmentError(f"required {target} tool is missing: {program}")
        result[name] = found
    if "cc" not in needed:
        return result
    if not microsoft:
        result["cc_args"] = ["--target=x86_64-w64-windows-gnu", "--sysroot=" + str(root)]
        return result
    # The GNU driver otherwise adds libcmt even when cc1 selected /MD.
    # Source objects already declare the matching CRT and startup libraries.
    result["cc_args"] = [
        "--target=x86_64-pc-windows-msvc",
        "-fms-runtime-lib=dll",
        "--start-no-unused-arguments",
        "-nostartfiles",
        "--end-no-unused-arguments",
    ]
    config = root / "msvc.toml"
    if args.native_sysroot or config.is_file():
        try:
            msvc = tomllib.loads(config.read_text(encoding="utf-8"))["msvc"]
            tools = root / msvc["vc_tools"]
            sdk = root / msvc["windows_sdk"]
            include = sdk / "Include" / msvc["sdk_version"]
            libraries = sdk / "Lib" / msvc["sdk_version"]
        except (KeyError, TypeError) as error:
            raise EnvironmentError(
                f"{config}: expected msvc.vc_tools, windows_sdk and sdk_version paths"
            ) from error
        for path in [
            tools / "include",
            *(include / part for part in ["ucrt", "shared", "um", "winrt"]),
        ]:
            result["cc_args"] += ["-idirafter", str(path)]
        for path in [tools / "lib/x64", libraries / "ucrt/x64", libraries / "um/x64"]:
            result["cc_args"] += ["-L", str(path)]
    return result
