#!/usr/bin/env python3
"""Prepare Scoop's vendored OpenSSL headers and static libraries for one target."""

import argparse
import json
import os
import platform
import subprocess
import tarfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION = "3.5.9"
TARGETS = {
    "aarch64-apple-darwin": ("darwin64-arm64-cc", "clang", "Darwin"),
    "x86_64-unknown-linux-gnu": ("linux-x86_64", "gcc", "Linux"),
    "x86_64-unknown-linux-musl": ("linux-x86_64", "musl-gcc", "Linux"),
}
OPTIONS = [
    "no-shared",
    "no-module",
    "no-dso",
    "no-engine",
    "no-autoload-config",
    "no-tests",
    "no-apps",
    "no-docs",
]


def metadata(path):
    value = path.stat()
    return [str(path), value.st_mtime_ns, value.st_size]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--build-dir", type=Path)
    parser.add_argument("--prefix", type=Path)
    parser.add_argument("--cc", help="Compiler for the selected target libc")
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 8))
    parser.add_argument("--rebuild", action="store_true")
    args = parser.parse_args()
    config, default_cc, host = TARGETS[args.target]
    if platform.system() != host:
        parser.error(f"prepare {args.target} on a {host} build host")
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    archive = ROOT / f"sysroot/vendor/openssl/openssl-{VERSION}.tar.gz"
    build = (args.build_dir or ROOT / "tmp/openssl" / args.target).resolve()
    prefix = (args.prefix or ROOT / "sysroot/lib" / args.target / "openssl").resolve()
    cc = args.cc or default_cc
    release = tomllib.loads((ROOT / "scripts/release.toml").read_text())
    flags = ["-fPIC"]
    if host == "Darwin":
        baseline = release["targets"][args.target]["minimum_os"]
        flags.append("-mmacosx-version-min=" + os.environ.get("MACOSX_DEPLOYMENT_TARGET", baseline))
    else:
        flags.append("-march=" + release["targets"][args.target]["cpu"])
    inputs = {
        "archive": metadata(archive),
        "script": metadata(Path(__file__).resolve()),
        "config": config,
        "cc": cc,
        "prefix": str(prefix),
        "options": OPTIONS + flags,
    }
    record = build / "inputs.json"
    outputs = [
        prefix / name for name in ("include/openssl/ssl.h", "lib/libssl.a", "lib/libcrypto.a")
    ]
    if (
        not args.rebuild
        and record.exists()
        and json.loads(record.read_text()) == inputs
        and all(path.exists() for path in outputs)
    ):
        print(f"Reusing {args.target} OpenSSL {VERSION}: {prefix}")
        return

    scratch = build / "scratch"
    scratch.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(scratch), CC=cc)
    sources = build / "source"
    source = sources / f"openssl-{VERSION}"
    # The complete upstream archive remains the source input for every target.
    with tarfile.open(archive) as upstream:
        upstream.extractall(sources, filter="data")
    objects = build / "objects"
    objects.mkdir(exist_ok=True)
    if (objects / "Makefile").exists():
        subprocess.run(["make", "clean"], cwd=objects, env=environment, check=True)
    command = [
        "perl",
        str(source / "Configure"),
        config,
        f"--prefix={prefix}",
        "--libdir=lib",
        *OPTIONS,
        *flags,
    ]
    subprocess.run(command, cwd=objects, env=environment, check=True)
    subprocess.run(
        ["make", f"-j{args.jobs}", "build_libs"], cwd=objects, env=environment, check=True
    )
    subprocess.run(["make", "install_dev"], cwd=objects, env=environment, check=True)
    record.write_text(json.dumps(inputs, indent=2) + "\n")
    print(f"Installed {args.target} OpenSSL {VERSION}: {prefix}")
    print(f"Default Scoop library directory: {prefix / 'lib'}")


if __name__ == "__main__":
    main()
