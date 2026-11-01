#!/usr/bin/env python3
"""Prepare the fixed Skia, HarfBuzz and ICU libraries for a supported GUI target."""

import argparse
import json
import os
import platform
import shutil
import subprocess
import tarfile
import tomllib
from pathlib import Path

import build_skia_windows
from build_support import (
    RELEASE_CONFIG,
    ROOT,
    apple_architecture,
    build_lock,
    cpu_flag,
    metadata,
    reusable,
    save_record,
    source_patch_environment,
)
from target_catalog import WINDOWS_TARGETS, native_configuration

REVISION = "f326a1e25fc736704063922b9ce76b527b1c5efe"
TARGETS = ("aarch64-apple-darwin", "x86_64-apple-darwin", *WINDOWS_TARGETS)
LIBRARY = "libscoop_skia.a"
PACKAGES = (
    (f"skia-{REVISION}", f"skia-{REVISION}", "."),
    (
        "harfbuzz-9cb1fee51069b206effb4736e443b038d230789d",
        "harfbuzz-9cb1fee51069b206effb4736e443b038d230789d",
        "third_party/externals/harfbuzz",
    ),
    ("icu-d578f2e8b7bd5938e21cfb6bf15c079e0aa5b738", "", "third_party/externals/icu"),
    (
        "libpng-d5515b5b8be3901aac04e5bd8bd5c89f287bcd33",
        "libpng-d5515b5b8be3901aac04e5bd8bd5c89f287bcd33",
        "third_party/externals/libpng",
    ),
    (
        "libjpeg-turbo-e14cbfaa85529d47f9f55b0f104a579c1061f9ad",
        "",
        "third_party/externals/libjpeg-turbo",
    ),
)
GN_TARGETS = ("skia", "skparagraph", "skshaper", "skunicode_core", "skunicode_icu")
ARCHIVES = (*GN_TARGETS, "harfbuzz", "icu", "png", "jpeg", "jpeg12", "jpeg16", "skcms")
HEADERS = (
    "include",
    "src/core/SkUTF.h",
    "modules/skcms/skcms.h",
    "modules/skcms/src/skcms_public.h",
    *(f"modules/{name}/include" for name in ("skparagraph", "skshaper", "skunicode")),
)
OPTIONS = {
    "is_official_build": True,
    "is_debug": False,
    "is_component_build": False,
    "target_os": "mac",
    "skia_enable_tools": False,
    "skia_enable_ganesh": True,
    "skia_enable_graphite": False,
    "skia_use_metal": True,
    "skia_use_gl": False,
    "skia_use_vulkan": False,
    "skia_use_dawn": False,
    "skia_use_partition_alloc": False,
    "skia_use_perfetto": False,
    "skia_enable_pdf": False,
    "skia_enable_svg": False,
    "skia_enable_skottie": False,
    "skia_use_expat": False,
    "skia_use_piex": False,
    "skia_use_dng_sdk": False,
    "skia_use_wuffs": False,
    "skia_use_fontconfig": False,
    "skia_use_freetype": False,
    "skia_use_harfbuzz": True,
    "skia_use_system_harfbuzz": False,
    "skia_use_icu": True,
    "skia_use_system_icu": False,
    "skia_use_libpng_decode": True,
    "skia_use_libpng_encode": False,
    "skia_use_no_png_encode": True,
    "skia_use_system_libpng": False,
    "skia_use_libjpeg_turbo_decode": True,
    "skia_use_libjpeg_turbo_encode": False,
    "skia_use_no_jpeg_encode": True,
    "skia_use_system_libjpeg_turbo": False,
    "skia_use_libwebp_decode": False,
    "skia_use_libwebp_encode": False,
    "skia_use_no_webp_encode": True,
    "skia_use_system_zlib": True,
}


def prepare(args):
    vendor = ROOT / "sysroot/vendor/skia"
    windows = args.target in WINDOWS_TARGETS
    patches = [
        vendor / "patches" / ("windows-toolchain.patch" if windows else "macos-deployment.patch"),
        vendor / "patches/text-baselines.patch",
    ]
    packages = (*PACKAGES, *build_skia_windows.PACKAGES) if windows else PACKAGES
    archives = [vendor / f"{name}.tar.xz" for name, _, _ in packages]
    sources = [metadata(path) for path in (*archives, *patches)]
    inputs = {
        "sources": sources,
        "scripts": [
            metadata(path)
            for path in (
                __file__,
                build_skia_windows.__file__,
                RELEASE_CONFIG,
                ROOT / "scripts/build_support.py",
            )
        ],
        "target": args.target,
        "configuration": native_configuration(args.target),
        "prefix": str(args.prefix),
        "gn": args.gn,
        "cc": args.cc,
        "cxx": args.cxx,
        "sdk": args.sdk,
        "options": OPTIONS,
        "environment": {
            name: os.environ.get(name)
            for name in (
                "PATH",
                "DEVELOPER_DIR",
                "SDKROOT",
                "MACOSX_DEPLOYMENT_TARGET",
                "VCINSTALLDIR",
                "WINDOWSSDKDIR",
                "INCLUDE",
                "LIB",
            )
        },
    }
    record = args.build_dir / "inputs.json"
    outputs = [args.prefix / LIBRARY, *(args.prefix / "include/skia" / path for path in HEADERS)]
    if reusable(record, inputs, outputs):
        print(f"Reusing {args.target} Skia {REVISION}: {args.prefix}", flush=True)
        return

    tools = [shutil.which(name) for name in (args.gn, "ninja", "git")]
    if any(tool is None for tool in tools):
        raise ValueError("Skia preparation requires GN, Ninja and Git")
    gn, ninja, git = tools
    release = tomllib.loads(RELEASE_CONFIG.read_text())["targets"][args.target]
    if windows:
        platform_options, platform_tools = build_skia_windows.configuration(args, release)
    else:
        platform_options, platform_tools = darwin_configuration(args, release)
    source = args.build_dir / "source"
    source_record = args.build_dir / "source.json"
    if not source_record.exists() or json.loads(source_record.read_text()) != sources:
        if source.exists():
            shutil.rmtree(source)
        for archive, (_, root, destination) in zip(archives, packages, strict=True):
            output = source / destination
            if root:
                with tarfile.open(archive) as package:
                    package.extractall(args.build_dir, filter="data")
                output.parent.mkdir(parents=True, exist_ok=True)
                (args.build_dir / root).rename(output)
            else:
                output.mkdir(parents=True, exist_ok=True)
                with tarfile.open(archive) as package:
                    package.extractall(output, filter="data")
        subprocess.run(
            [git, "apply", *map(str, patches)],
            cwd=source,
            env=source_patch_environment(source),
            check=True,
        )
        source_record.write_text(json.dumps(sources) + "\n")

    options = dict(OPTIONS, **platform_options)
    objects = args.build_dir / "objects"
    objects.mkdir(parents=True, exist_ok=True)
    (objects / "args.gn").write_text(
        "".join(f"{name} = {json.dumps(value)}\n" for name, value in options.items())
    )
    subprocess.run([gn, "gen", str(objects)], cwd=source, check=True)
    subprocess.run([ninja, "-C", str(objects), "-j", str(args.jobs), *GN_TARGETS], check=True)
    staged = args.build_dir / LIBRARY
    if windows:
        build_skia_windows.combine(
            args, objects, staged, (*ARCHIVES, *build_skia_windows.ARCHIVES), platform_tools[-1]
        )
    else:
        subprocess.run(
            [
                platform_tools[-1],
                "-static",
                "-o",
                str(staged),
                *(str(objects / f"lib{name}.a") for name in ARCHIVES),
            ],
            check=True,
        )
    args.prefix.mkdir(parents=True, exist_ok=True)
    headers = args.prefix / "include/skia"
    for relative in HEADERS:
        origin = source / relative
        destination = headers / relative
        if origin.is_dir():
            shutil.copytree(origin, destination, dirs_exist_ok=True)
        else:
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(origin, destination)
    temporary = args.prefix / (LIBRARY + ".new")
    shutil.copy2(staged, temporary)
    temporary.replace(args.prefix / LIBRARY)
    save_record(record, inputs, objects, tools=[*tools, *platform_tools])
    print(f"Prepared {args.target} Skia {REVISION}: {args.prefix}", flush=True)


def darwin_configuration(args, release):
    tools = [
        shutil.which(name)
        for name in (args.cc or "/usr/bin/clang", args.cxx or "/usr/bin/clang++", "xcrun")
    ]
    if any(tool is None for tool in tools):
        raise ValueError("Darwin Skia preparation requires Apple Clang and Xcode")
    cc, cxx, xcrun = tools
    sdk = subprocess.check_output([xcrun, "--sdk", args.sdk, "--show-sdk-path"], text=True).strip()
    libtool = subprocess.check_output([xcrun, "--find", "libtool"], text=True).strip()
    architecture = native_configuration(args.target)["arch"]
    deployment = os.environ.get("MACOSX_DEPLOYMENT_TARGET") or release["minimum_os"]
    triple = f"{apple_architecture(architecture)}-apple-macos{deployment}"
    flags = ["-target", triple, "-isysroot", sdk]
    return {
        "target_cpu": "arm64" if architecture == "aarch64" else "x64",
        "cc": cc,
        "cxx": cxx,
        "mac_min_target": deployment,
        "xcode_sysroot": sdk,
        "extra_cflags": [*flags, cpu_flag(architecture, release["cpu"])],
        "extra_asmflags": flags,
        "extra_ldflags": flags,
    }, [*tools, libtool]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--build-dir", type=Path, required=True)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--gn", default="gn")
    parser.add_argument("--cc")
    parser.add_argument("--cxx")
    parser.add_argument("--sdk", default="macosx")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    host = "Windows" if args.target in WINDOWS_TARGETS else "Darwin"
    if platform.system() != host:
        parser.error(f"prepare {args.target} Skia resources on a {host} host")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    args.build_dir = args.build_dir.resolve()
    args.prefix = (args.prefix or ROOT / "sysroot/lib" / args.target).resolve()
    args.build_dir.mkdir(parents=True, exist_ok=True)
    with build_lock(args.build_dir / "build.lock"):
        prepare(args)


if __name__ == "__main__":
    main()
