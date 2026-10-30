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

from target_catalog import TARGET_FACTS, TARGETS

ROOT = Path(__file__).resolve().parent.parent


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


def has_target_sources(manifest, target):
    sources = manifest.get("sources")
    if not sources:
        return True
    facts = TARGET_FACTS[target]
    for source in sources:
        matches = True
        for key, values in source.get("when", {}).items():
            if key not in facts:
                raise ValueError(f"unknown target predicate {key}")
            if isinstance(values, str):
                values = [values]
            if not isinstance(values, list) or any(not isinstance(v, str) for v in values):
                raise ValueError(f"target predicate {key} requires strings")
            known = {entry[key] for entry in TARGET_FACTS.values()}
            if any(value not in known for value in values):
                raise ValueError(f"unknown target predicate {key} value in {values}")
            matches = matches and facts[key] in values
        if matches:
            return True
    return False


def cone_order(sysroot, work, target):
    manifests = sorted((sysroot / "src").glob("*/Cone.toml"))
    inputs = [metadata(path) for path in manifests]
    inputs.append(metadata(Path(__file__).resolve()))
    inputs.append({"target_facts": TARGET_FACTS})
    record = work / "cones.json"
    if record.exists():
        saved = json.loads(record.read_text())
        if saved.get("target") == target and saved["inputs"] == inputs:
            return saved["order"]

    cones, dependencies = {}, {}
    for path in manifests:
        with path.open("rb") as source:
            manifest = tomllib.load(source)
        if not has_target_sources(manifest, target):
            continue
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
    save_record(record, {"target": target, "inputs": inputs, "order": order})
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


def build_cones(sysroot, target, names, binaries, work, cache, jobs, environment, *, native=()):
    for name in names:
        print(f"Building {name} for {target}", flush=True)
        subprocess.run(
            [
                binaries[0],
                "build",
                sysroot / "src" / name,
                "--release",
                "--jobs",
                str(jobs),
                "--target",
                target,
                "--sysroot",
                sysroot,
                "--scoopc",
                binaries[1],
                "--cache-dir",
                cache,
                "--target-dir",
                work / "target" / name,
                "--lockfile",
                work / "locks" / f"{name}.lock",
                "-o",
                sysroot / "lib" / target / f"{name}.slib",
                *native,
            ],
            env=environment,
            check=True,
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--scoop", type=Path)
    parser.add_argument("--scoopc", type=Path)
    parser.add_argument("--sysroot", type=Path, default=ROOT / "sysroot")
    parser.add_argument("--runtime-root", type=Path, default=ROOT / "runtime")
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--cache-dir", type=Path)
    parser.add_argument("--cc", type=Path)
    parser.add_argument("--native-sysroot", type=Path)
    parser.add_argument("--unwind-prefix", type=Path)
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    parser.add_argument(
        "--runtime-only",
        action="store_true",
        help="Prepare runtime and public headers without building Cones",
    )
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    if not args.runtime_only and args.scoop is None:
        parser.error("--scoop is required unless --runtime-only is used")
    args.sysroot = args.sysroot.resolve()
    args.runtime_root = args.runtime_root.resolve()
    work = (args.work_dir or ROOT / "tmp/sysroot" / args.target).resolve()
    work.mkdir(parents=True, exist_ok=True)
    args.cache_dir = (args.cache_dir or work / "cache").resolve()
    scratch = work / "scratch"
    scratch.mkdir(exist_ok=True)
    environment = dict(os.environ, TMPDIR=str(scratch), TMP=str(scratch), TEMP=str(scratch))
    if args.unwind_prefix is None and TARGETS[args.target]["os"] != "darwin":
        args.unwind_prefix = args.sysroot / "lib" / args.target / "unwind"
    native = []
    for flag, value in (
        ("--cc", args.cc),
        ("--native-sysroot", args.native_sysroot),
        ("--unwind-prefix", args.unwind_prefix),
    ):
        if value is not None:
            native.extend([flag, str(value)])

    publish_files(
        args.runtime_root / "include",
        args.sysroot / "include",
        [Path("scoop_rt.h"), Path("scoop_runtime_metadata_v1.h")],
        work / "headers.json",
    )
    if not args.runtime_only:
        scoop = args.scoop.resolve()
        scoopc = (
            args.scoopc or scoop.with_name("scoopc.exe" if os.name == "nt" else "scoopc")
        ).resolve()
        build_cones(
            args.sysroot,
            args.target,
            cone_order(args.sysroot, work, args.target),
            [scoop, scoopc],
            work,
            args.cache_dir,
            args.jobs,
            environment,
            native=native,
        )
    for enable_gc_stress in (False, True):
        for enable_startup_stats in (False, True):
            runtime(args, work, environment, native, enable_gc_stress, enable_startup_stats)
    print(
        f"Prepared {'runtime' if args.runtime_only else 'system libraries and runtime'} for {args.target}"
    )


if __name__ == "__main__":
    main()
