"""Build the shared Rust text archive from locked, vendored dependencies."""

import json
import os
import shutil
import subprocess
import tarfile
import tomllib
from pathlib import Path

from build_support import RELEASE_CONFIG, ROOT, build_lock, metadata, reusable, source_metadata
from target_catalog import TARGETS, WINDOWS_TARGETS, native_configuration

LIBRARY = "libscoop_text_backend.a"
HEADER = "scoop_text_backend.h"
NOTICE_FILES = ("COPYRIGHT-library.html", "licenses/Apache-2.0.txt", "licenses/MIT.txt")
NOTICE_DIRECTORY = "rust-std"


def prepare(args):
    source = ROOT / "sysroot/native/text_backend"
    archive = ROOT / "sysroot/vendor/text_backend/crates.tar.gz"
    build = args.build_dir / "rust"
    build.mkdir(parents=True, exist_ok=True)
    with build_lock(build / "build.lock"):
        prepare_locked(args, source, archive, build)


def prepare_locked(args, source, archive, build):
    rust_target = TARGETS[args.target]["native_rust_target"]
    inputs = {
        "source": source_metadata(source, ("src", "include")),
        "files": [
            metadata(path)
            for path in (
                source / "Cargo.toml",
                source / "Cargo.lock",
                archive,
                Path(__file__),
                ROOT / "scripts/build_support.py",
                RELEASE_CONFIG,
            )
        ],
        "target": args.target,
        "target_configuration": native_configuration(args.target),
        "rust_target": rust_target,
        "prefix": str(args.prefix),
        "cc": args.cc,
        "environment": {
            name: os.environ.get(name)
            for name in (
                "PATH",
                "RUSTUP_TOOLCHAIN",
                "RUSTFLAGS",
                "CARGO_ENCODED_RUSTFLAGS",
                "MACOSX_DEPLOYMENT_TARGET",
            )
        },
    }
    record = build / "inputs.json"
    outputs = [args.prefix / LIBRARY, args.prefix / "include" / HEADER]
    outputs += [args.prefix / NOTICE_DIRECTORY / Path(name).name for name in NOTICE_FILES]
    if reusable(record, inputs, outputs):
        print(f"Reusing {args.target} Rust text backend", flush=True)
        return

    cargo = shutil.which("cargo")
    rustc = shutil.which("rustc")
    if not cargo or not rustc:
        raise ValueError("preparing the Rust text backend requires Cargo and rustc")
    configuration = tomllib.loads(RELEASE_CONFIG.read_text())
    version = subprocess.check_output([rustc, "--version"], text=True)
    if args.target in WINDOWS_TARGETS and version.split()[1] != configuration["rust_windows"]:
        raise ValueError("Windows text preparation requires Rust " + configuration["rust_windows"])
    rust_sysroot = Path(subprocess.check_output([rustc, "--print=sysroot"], text=True).strip())
    notices = [rust_sysroot / "share/doc/rust" / name for name in NOTICE_FILES]
    for notice in notices:
        if not notice.is_file():
            raise ValueError(f"Rust standard library license document is missing: {notice}")
    tools = [Path(cargo), Path(rustc)]
    tools += notices
    # rustup's selected compiler can change without replacing its proxy binaries.
    rustup = shutil.which("rustup")
    if rustup:
        for name in ("cargo", "rustc"):
            selected = subprocess.check_output([rustup, "which", name], text=True).strip()
            tools.append(Path(selected))
        settings = Path(os.environ.get("RUSTUP_HOME", Path.home() / ".rustup")) / "settings.toml"
        if settings.exists():
            tools.append(settings)

    scratch = build / "scratch"
    scratch.mkdir(exist_ok=True)
    vendor = build / "vendor"
    vendor.mkdir(exist_ok=True)
    vendor_record = build / "vendor.json"
    archive_input = metadata(archive)
    if not vendor_record.exists() or json.loads(vendor_record.read_text()) != archive_input:
        if (vendor / "crates").exists():
            shutil.rmtree(vendor / "crates")
        with tarfile.open(archive) as package:
            package.extractall(vendor, filter="data")
        vendor_record.write_text(json.dumps(archive_input) + "\n")

    release = configuration["targets"][args.target]
    environment = dict(
        os.environ,
        TMPDIR=str(scratch),
        TEMP=str(scratch),
        TMP=str(scratch),
        CARGO_TARGET_DIR=str(build / "target"),
    )
    if TARGETS[args.target]["os"] == "darwin":
        environment["MACOSX_DEPLOYMENT_TARGET"] = (
            inputs["environment"]["MACOSX_DEPLOYMENT_TARGET"] or release["minimum_os"]
        )
    command = [
        cargo,
        "--config",
        'source.crates-io.replace-with="scoop-vendor"',
        "--config",
        "source.scoop-vendor.directory=" + json.dumps(str(vendor / "crates")),
        "rustc",
        "--manifest-path",
        str(source / "Cargo.toml"),
        "--locked",
        "--offline",
        "--lib",
        "--release",
        "--target",
        rust_target,
        "--jobs",
        str(args.jobs),
        "--",
        "--print=native-static-libs",
        "-Ctarget-cpu=" + release["cpu"],
    ]
    subprocess.run(command, env=environment, check=True)
    (build / "toolchain.txt").write_text(version)
    stage = build / "stage"
    stage.mkdir(exist_ok=True)
    built_name = "scoop_text_backend.lib" if rust_target.endswith("-msvc") else LIBRARY
    shutil.copy2(build / "target" / rust_target / "release" / built_name, stage / LIBRARY)
    shutil.copy2(source / "include" / HEADER, stage / HEADER)
    staged_files = [stage / LIBRARY, stage / HEADER]
    for notice in notices:
        shutil.copy2(notice, stage / notice.name)
        staged_files.append(stage / notice.name)
    for staged, output in zip(staged_files, outputs, strict=True):
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary = output.with_suffix(output.suffix + ".new")
        shutil.copy2(staged, temporary)
        temporary.replace(output)
    state = {"inputs": inputs, "dependencies": [metadata(path) for path in tools]}
    temporary = record.with_suffix(".new")
    temporary.write_text(json.dumps(state, separators=(",", ":")) + "\n")
    temporary.replace(record)
    print(f"Prepared {args.target} Rust text backend ({version.strip()})", flush=True)
