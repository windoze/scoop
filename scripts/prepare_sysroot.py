#!/usr/bin/env python3
"""Prepare one development or release sysroot using ordinary Scoop builds."""

import argparse
import graphlib
import json
import os
import shutil
import subprocess
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "x86_64-pc-windows-msvc-ucrt",
    "x86_64-w64-windows-llvm-mingw-msvcrt",
    "x86_64-w64-windows-llvm-mingw-ucrt",
)


def host_path(carrier):
    data = bytes(carrier["bytes"])
    if carrier["encoding"] == "UnixBytes":
        return Path(os.fsdecode(data))
    if carrier["encoding"] == "WindowsWtf16Le":
        return Path(data.decode("utf-16-le", errors="surrogatepass"))
    raise ValueError(f"unknown runtime builder path encoding: {carrier['encoding']}")


def metadata(path):
    value = path.stat()
    return [str(path), value.st_mtime_ns, value.st_size]


def save_record(path, value):
    staged = path.with_suffix(".new")
    staged.write_text(json.dumps(value, indent=2) + "\n")
    staged.replace(path)


def cone_order(sysroot, work):
    manifests = sorted((sysroot / "src").glob("*/Cone.toml"))
    inputs = [metadata(path) for path in manifests]
    inputs.append(metadata(Path(__file__).resolve()))
    record = work / "cones.json"
    if record.exists():
        saved = json.loads(record.read_text())
        if saved["inputs"] == inputs:
            return saved["order"]

    cones, dependencies = {}, {}
    for path in manifests:
        with path.open("rb") as source:
            manifest = tomllib.load(source)
        cone = manifest["cone"]
        name = f"{cone['group']}:{cone['name']}"
        if name in cones:
            raise ValueError(f"duplicate sysroot Cone {name}")
        cones[name] = path.parent.name
        dependencies[name] = list(manifest.get("dependencies", {}))
        if name != "scoop:scoop.core" and "scoop:scoop.core" not in dependencies[name]:
            dependencies[name].append("scoop:scoop.core")
        dependencies[name].sort()
    if not cones:
        raise ValueError(f"no Cone manifests under {sysroot / 'src'}")
    order = []
    for name in graphlib.TopologicalSorter(dependencies).static_order():
        if name not in cones:
            raise ValueError(f"sysroot dependency {name} has no source Cone")
        order.append(cones[name])
    save_record(record, {"inputs": inputs, "order": order})
    return order


def publish_files(source, destination, files, record):
    """Stage known outputs before replacing a prepared directory."""
    destination = destination.resolve()
    inputs = [metadata(source / relative) for relative in files]
    state = {"inputs": inputs, "destination": str(destination)}
    if (
        record.exists()
        and json.loads(record.read_text()) == state
        and all((destination / relative).exists() for relative in files)
    ):
        print(f"Reusing {destination}", flush=True)
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".prepare-", dir=destination.parent) as temporary:
        staged = Path(temporary) / "new"
        staged.mkdir()
        for relative in files:
            output = staged / relative
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source / relative, output)
        previous = Path(temporary) / "previous"
        if destination.exists():
            destination.rename(previous)
        try:
            staged.rename(destination)
        except OSError:
            if previous.exists():
                previous.rename(destination)
            raise
    save_record(record, state)
    print(f"Prepared {destination}", flush=True)


def runtime(args, work, environment, native, enable_gc_stress=False, enable_startup_stats=False):
    command = [
        "cargo",
        "run",
        "--quiet",
        "--release",
        "--locked",
        "--manifest-path",
        str(ROOT / "scripts/runtime_builder/Cargo.toml"),
        "--target-dir",
        environment.get("CARGO_TARGET_DIR", str(work / "rust")),
        "--",
        "--target",
        args.target,
        "--runtime-root",
        str(args.runtime_root),
        "--vendor-root",
        str(args.sysroot / "vendor"),
        "--cache-dir",
        str(args.cache_dir),
        *native,
    ]
    if enable_gc_stress:
        command.append("--enable-gc-stress")
    if enable_startup_stats:
        command.append("--enable-startup-stats")
    result = subprocess.run(command, env=environment, check=True, stdout=subprocess.PIPE)
    output = json.loads(result.stdout)
    source = host_path(output["root"])
    files = [host_path(path) for path in output["files"]]
    variant = "runtime"
    if enable_gc_stress:
        variant += "-stress"
    if enable_startup_stats:
        variant += "-startup-stats"
    publish_files(
        source,
        args.sysroot / "lib" / args.target / variant,
        files,
        work / f"{variant}.json",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--scoop", type=Path, required=True)
    parser.add_argument("--scoopc", type=Path)
    parser.add_argument("--sysroot", type=Path, default=ROOT / "sysroot")
    parser.add_argument("--runtime-root", type=Path, default=ROOT / "runtime")
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--cache-dir", type=Path)
    parser.add_argument("--cc", type=Path)
    parser.add_argument("--native-sysroot", type=Path)
    parser.add_argument("--unwind-prefix", type=Path)
    args = parser.parse_args()
    args.sysroot = args.sysroot.resolve()
    args.runtime_root = args.runtime_root.resolve()
    work = (args.work_dir or ROOT / "tmp/sysroot" / args.target).resolve()
    work.mkdir(parents=True, exist_ok=True)
    args.cache_dir = (args.cache_dir or work / "cache").resolve()
    scratch = work / "scratch"
    scratch.mkdir(exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(scratch))
    scoop = args.scoop.resolve()
    scoopc = (
        args.scoopc or scoop.with_name("scoopc.exe" if os.name == "nt" else "scoopc")
    ).resolve()
    if args.unwind_prefix is None and args.target != "aarch64-apple-darwin":
        args.unwind_prefix = args.sysroot / "lib" / args.target / "unwind"
    native = []
    for flag, value in (
        ("--cc", args.cc),
        ("--native-sysroot", args.native_sysroot),
        ("--unwind-prefix", args.unwind_prefix),
    ):
        if value is not None:
            native.extend([flag, str(value)])

    order = cone_order(args.sysroot, work)
    publish_files(
        args.runtime_root / "include",
        args.sysroot / "include",
        [Path("scoop_rt.h"), Path("scoop_runtime_metadata_v1.h")],
        work / "headers.json",
    )
    for name in order:
        print(f"Building {name} for {args.target}", flush=True)
        subprocess.run(
            [
                scoop,
                "build",
                args.sysroot / "src" / name,
                "--release",
                "--target",
                args.target,
                "--sysroot",
                args.sysroot,
                "--scoopc",
                scoopc,
                "--cache-dir",
                args.cache_dir,
                "--target-dir",
                work / "target" / name,
                "--lockfile",
                work / "locks" / f"{name}.lock",
                "-o",
                args.sysroot / "lib" / args.target / f"{name}.slib",
                *native,
            ],
            env=environment,
            check=True,
        )
    for enable_gc_stress in (False, True):
        for enable_startup_stats in (False, True):
            runtime(args, work, environment, native, enable_gc_stress, enable_startup_stats)
    print(f"Prepared {len(order)} Cones and runtime for {args.target}")


if __name__ == "__main__":
    main()
