#!/usr/bin/env python3
"""Prepare fixed iconv, Unicode and IDNA static libraries for one Scoop target."""

import argparse
import json
import os
import platform
import shutil
import subprocess
import tarfile
import tomllib
from pathlib import Path

from build_support import RELEASE_CONFIG, ROOT, metadata, reusable, save_record, source_metadata

PACKAGES = (
    ("libiconv", "1.19", ".tar.gz", ("libiconv.a", "libcharset.a"), "iconv.h"),
    ("libunistring", "1.4.2", ".tar.xz", ("libunistring.a",), "unistr.h"),
    ("libidn2", "2.3.8", ".tar.gz", ("libidn2.a",), "idn2.h"),
)
TARGETS = {
    "aarch64-apple-darwin": ("Darwin", "clang"),
    "x86_64-unknown-linux-gnu": ("Linux", "gcc"),
    "x86_64-unknown-linux-musl": ("Linux", "musl-gcc"),
}


def dependency_headers(prefix, name):
    if name == "libiconv":
        return []
    names = ["iconv.h", "libcharset.h", "localcharset.h"]
    nested = []
    if name == "libidn2":
        names += ["uniconv.h", "uninorm.h", "unistr.h", "unictype.h", "unitypes.h", "unimetadata.h"]
        nested = source_metadata(prefix, ("include/unistring",))
    return [metadata(prefix / "include" / name) for name in names] + nested


def prepare(package, args, cc, make, dependencies):
    name, version, extension, libraries, header = package
    vendor = ROOT / "sysroot/vendor" / name
    archive = vendor / f"{name}-{version}{extension}"
    patches = sorted((vendor / "patches").glob("*.patch"))
    build = args.build_dir / name
    prefix = args.prefix
    inputs = {
        "archive": metadata(archive),
        "patches": [metadata(path) for path in patches],
        "tools": [metadata(path) for path in (cc, make, __file__, RELEASE_CONFIG)],
        "support": metadata(ROOT / "scripts/build_support.py"),
        "target": args.target,
        "prefix": str(prefix),
        "dependencies": [metadata(prefix / library) for library in dependencies],
        "headers": dependency_headers(prefix, name),
        "deployment": os.environ.get("MACOSX_DEPLOYMENT_TARGET"),
    }
    if patches:
        inputs["patch_tool"] = metadata(shutil.which("patch") or "/usr/bin/patch")
    outputs = [prefix / library for library in libraries] + [prefix / "include" / header]
    record = build / "inputs.json"
    if reusable(record, inputs, outputs):
        print(f"Reusing {args.target} {name} {version}", flush=True)
        return

    build.mkdir(parents=True, exist_ok=True)
    source = build / f"{name}-{version}"
    source_record = build / "source.json"
    source_inputs = {key: inputs[key] for key in ("archive", "patches")}
    if not source_record.exists() or json.loads(source_record.read_text()) != source_inputs:
        if source.exists():
            shutil.rmtree(source)
        with tarfile.open(archive) as package:
            package.extractall(build, filter="data")
        for patch in patches:
            subprocess.run(["patch", "--batch", "-p1", "-i", str(patch)], cwd=source, check=True)
        source_record.write_text(json.dumps(source_inputs) + "\n")

    objects = build / "objects"
    stage = build / "stage"
    for directory in (objects, stage):
        if directory.exists():
            shutil.rmtree(directory)
        directory.mkdir()
    scratch = build / "scratch"
    scratch.mkdir(exist_ok=True)
    release = tomllib.loads(RELEASE_CONFIG.read_text())["targets"][args.target]
    flags = "-O2 -fPIC " + (
        "-mcpu=generic" if args.target.endswith("darwin") else "-march=" + release["cpu"]
    )
    if name != "libiconv":
        flags += f" -Dlocale_charset=scoop_{name}_locale_charset"
    if name == "libidn2":
        flags += " -D_UC_CATEGORY_NONE=scoop_libidn2_uc_category_none"
    environment = dict(
        os.environ,
        TMPDIR=str(scratch),
        CC=cc,
        CFLAGS=flags,
        CPPFLAGS="-I" + str(prefix / "include"),
        LDFLAGS="-L" + str(prefix),
    )
    if args.target.endswith("darwin"):
        environment["MACOSX_DEPLOYMENT_TARGET"] = inputs["deployment"] or release["minimum_os"]
    options = [
        f"--prefix={prefix}",
        f"--libdir={prefix}",
        "--disable-shared",
        "--enable-static",
        "--disable-rpath",
    ]
    if name != "libunistring":
        options.append("--disable-nls")
    if name != "libiconv":
        options.append(f"--with-libiconv-prefix={prefix}")
    if name == "libidn2":
        options += [f"--with-libunistring-prefix={prefix}", "--disable-doc"]
    for command in (
        [str(source / "configure"), *options],
        [make, "-j", str(args.jobs)],
        [make, "install", f"DESTDIR={stage}"],
    ):
        subprocess.run(command, cwd=objects, env=environment, check=True)

    installed = stage / prefix.relative_to(prefix.anchor)
    prefix.mkdir(parents=True, exist_ok=True)
    for library in libraries:
        shutil.copy2(installed / library, prefix / library)
    shutil.copytree(installed / "include", prefix / "include", dirs_exist_ok=True)
    save_record(record, inputs, objects)
    print(f"Prepared {args.target} {name} {version}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--build-dir", type=Path, required=True)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--cc")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    host, default_cc = TARGETS[args.target]
    if platform.system() != host:
        parser.error(f"prepare {args.target} on a {host} build host")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    cc, make = shutil.which(args.cc or default_cc), shutil.which("make")
    if not cc or not make:
        parser.error("the selected C compiler and make must be installed")
    args.build_dir = args.build_dir.resolve()
    args.prefix = (args.prefix or ROOT / "sysroot/lib" / args.target).resolve()
    dependencies = []
    for package in PACKAGES:
        prepare(package, args, cc, make, dependencies)
        dependencies.extend(package[3])


if __name__ == "__main__":
    main()
