#!/usr/bin/env python3
"""Build a complete Scoop archive, including sysroot, VS Code tooling, and programming skill."""

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from pathlib import Path

import build_scoop_example
import package_debugger
import package_documentation
import package_licenses
import package_llvm
import package_native
import package_tooling
import package_windows
from build_llvm import HOSTS, VERSION
from build_rust_text import HEADER as RUST_TEXT_HEADER
from build_rust_text import LIBRARY as RUST_TEXT_LIBRARY
from build_support import RELEASE_CONFIG, ROOT
from compression_backends import PACKAGES as COMPRESSION_PACKAGES

TOOLS = ("scoop", "scoopc", "scoop-link", "scoop-lsp", "scoop-pm")


def run_script(name, arguments, environment):
    subprocess.run(
        [sys.executable, str(ROOT / "scripts" / name), *map(str, arguments)],
        check=True,
        env=environment,
    )


def prepare_targets(args, targets, binaries, environment):
    sysroot = ROOT / "sysroot"
    for target in targets:
        native = []
        openssl = [
            "--target",
            target,
            "--build-dir",
            args.work_dir / "openssl" / target,
            "--jobs",
            args.jobs,
        ]
        if "linux" in target:
            cc = args.musl_cc if target.endswith("musl") else args.gnu_cc
            native = ["--cc", cc]
            openssl += native
        elif "windows" in target:
            native = ["--cc", package_windows.compiler(args, target)]
            openssl += [*native, *package_windows.openssl_arguments(args, target)]
        run_script("build_openssl.py", openssl, environment)
        for script, directory in (
            ("build_nghttp2.py", "nghttp2"),
            ("build_text_backends.py", "text"),
            ("build_uriparser.py", "uriparser"),
            ("build_compression.py", "compression"),
            ("build_pcre2.py", "pcre2"),
        ):
            run_script(
                script,
                [
                    "--target",
                    target,
                    "--build-dir",
                    args.work_dir / directory / target,
                    "--jobs",
                    args.jobs,
                    *native,
                ],
                environment,
            )
        if "linux" in target:
            unwind = [
                "--target",
                target,
                "--source",
                args.llvm_source,
                "--build-dir",
                args.work_dir / "unwind" / target,
                "--cc",
                args.unwind_cc,
                "--cxx",
                args.unwind_cxx,
                "--jobs",
                args.jobs,
            ]
            if target.endswith("musl"):
                unwind += ["--musl-include", args.musl_include]
            run_script("build_llvm_unwind.py", unwind, environment)
            native += ["--unwind-prefix", sysroot / "lib" / target / "unwind"]
        elif "windows" in target:
            run_script(
                "build_windows_unwind.py",
                [
                    "--target",
                    target,
                    "--source",
                    args.llvm_source,
                    "--build-dir",
                    args.work_dir / "unwind" / target,
                    "--jobs",
                    args.jobs,
                    *native,
                ],
                environment,
            )
            native += ["--unwind-prefix", sysroot / "lib" / target / "unwind"]
            prefix = package_windows.mingw_prefix(args, target)
            if prefix is not None:
                native += ["--native-sysroot", prefix]
        run_script(
            "prepare_sysroot.py",
            [
                "--target",
                target,
                "--scoop",
                binaries[0],
                "--scoopc",
                binaries[1],
                "--sysroot",
                sysroot,
                "--work-dir",
                args.work_dir / "sysroot" / target,
                "--cache-dir",
                args.work_dir / "cache",
                "--jobs",
                args.jobs,
                *native,
            ],
            environment,
        )


def assemble(root, targets, binaries, args, release, extension):
    sysroot = args.prepared_sysroot or ROOT / "sysroot"
    (root / "bin").mkdir(parents=True)
    for binary in binaries:
        name = "sco.exe" if "windows" in args.host and binary.name == "scoop.exe" else binary.name
        shutil.copy2(binary, root / "bin" / name)
    (root / "VERSION").write_text(
        f"{args.version}\nrevision: {args.revision}\nhost: {args.host}\n", encoding="utf-8"
    )
    (root / "tooling").mkdir()
    shutil.copy2(extension, root / "tooling" / extension.name)
    shutil.copytree(ROOT / "tooling/scoop-programming", root / "tooling/scoop-programming")
    for name in ("src", "include"):
        shutil.copytree(
            sysroot / name, root / name, ignore=shutil.ignore_patterns("target", "Cone.lock")
        )
    shutil.copytree(
        sysroot / "commands", root / "commands", ignore=shutil.ignore_patterns(".gitkeep")
    )
    shutil.copytree(
        sysroot / "native/text_backend",
        root / "native/text_backend",
        ignore=shutil.ignore_patterns("target"),
    )
    for vendor in (sysroot / "vendor").iterdir():
        if not vendor.is_dir():
            continue
        destination = root / "vendor" / vendor.name
        if vendor.name == "llvm-project":
            version = vendor / VERSION
            shutil.copytree(version / "patches", destination / VERSION / "patches")
            shutil.copy2(version / "README.md", destination / VERSION / "README.md")
        else:
            shutil.copytree(vendor, destination)
    cones = [
        tomllib.loads(path.read_text())["cone"]
        for path in sorted((sysroot / "src").glob("*/Cone.toml"))
    ]
    for target in targets:
        source = sysroot / "lib" / target
        destination = root / "lib" / target
        destination.mkdir(parents=True)
        for cone in cones:
            shutil.copy2(source / f"{cone['name']}.slib", destination)
        (destination / "include").mkdir()
        for header in (
            RUST_TEXT_HEADER,
            "uriparser",
            "pcre2.h",
            "zstd.h",
            "zstd_errors.h",
            "zdict.h",
            "zlib-ng.h",
            "zconf-ng.h",
            "zlib_name_mangling-ng.h",
            "brotli",
        ):
            origin = source / "include" / header
            output = destination / "include" / header
            if origin.is_dir():
                shutil.copytree(origin, output)
            else:
                shutil.copy2(origin, output)
        shutil.copy2(source / RUST_TEXT_LIBRARY, destination / RUST_TEXT_LIBRARY)
        shutil.copy2(source / "liburiparser.a", destination / "liburiparser.a")
        shutil.copy2(source / "libpcre2-8.a", destination / "libpcre2-8.a")
        for package in COMPRESSION_PACKAGES:
            for library in package["libraries"]:
                shutil.copy2(source / library, destination / library)
        for variant in (
            "runtime",
            "runtime-stress",
            "runtime-startup-stats",
            "runtime-stress-startup-stats",
        ):
            shutil.copytree(source / variant, destination / variant)
        for component, libraries in (
            ("openssl", ("libcrypto.a", "libssl.a")),
            ("nghttp2", ("libnghttp2.a",)),
            ("unwind", ("libunwind.a",)),
        ):
            if component == "unwind" and target.endswith("darwin"):
                continue
            shutil.copytree(source / component / "include", destination / component / "include")
            (destination / component / "lib").mkdir()
            for name in libraries:
                shutil.copy2(
                    source / component / "lib" / name, destination / component / "lib" / name
                )
    debugger_files, python_version, python_license = package_debugger.stage(
        root,
        args.llvm_prefix,
        args.debugger_prefix,
        args.debugger_python,
        args.host,
        args.python_license,
        windows_compiler=args.mingw_ucrt / "bin/clang.exe" if "windows" in args.host else None,
    )
    if "windows" in args.host:
        package_windows.install(root, targets, args)
        native = package_native.windows(root, binaries, args.llvm_prefix / "bin", debugger_files)
    else:
        native = (package_native.darwin if args.host.endswith("darwin") else package_native.linux)(
            root, binaries, args.llvm_prefix / "lib", debugger_files
        )
    package_licenses.collect(
        root,
        ROOT,
        args.llvm_source,
        args.host,
        native,
        targets,
        sysroot=sysroot,
        python_license=python_license,
    )
    share = root / "share/scoop"
    share.mkdir(parents=True, exist_ok=True)
    shutil.copytree(sysroot / "share/scoop/examples", share / "examples")
    for name in ("README.md", "RELEASE-NOTES.md"):
        shutil.copy2(ROOT / "scripts/distribution" / name, share / name)
    manifest = [
        "schema = 1",
        f"version = {json.dumps(args.version)}",
        f"source_revision = {json.dumps(args.revision)}",
        f"host = {json.dumps(args.host)}",
        f"vscode_extension = {json.dumps('tooling/' + extension.name)}",
        'programming_skill = "tooling/scoop-programming"',
        f"llvm = {json.dumps(release['llvm'])}",
        f"lldb = {json.dumps(release['llvm'])}",
        f"debugger_python = {json.dumps(python_version)}",
        'debugger_formatters = "share/scoop/debugger/scoop_lldb.py"',
        f"openssl = {json.dumps(release['openssl'])}",
        f"llhttp = {json.dumps(release['llhttp'])}",
        f"nghttp2 = {json.dumps(release['nghttp2'])}",
        f"uriparser = {json.dumps(release['uriparser'])}",
        f"pcre2 = {json.dumps(release['pcre2'])}",
        f"host_libraries = {json.dumps(sorted(path.name for path in native))}",
    ]
    if "windows" in args.host:
        manifest += [
            f"{name} = {json.dumps(release[name])}" for name in ("rust_windows", "llvm_mingw")
        ]
    manifest += [
        f"{name} = {json.dumps(release[name])}"
        for name in ("text_backend", "encoding_rs", "idna", "icu4x", "unicode", "unicode-width")
    ]
    manifest += [
        f"{package['name']} = {json.dumps(release[package['name']])}"
        for package in COMPRESSION_PACKAGES
    ]
    uring = (sysroot / "vendor/liburing/2.15/liburing.spec").read_text()
    uring_version = re.search(r"^Version:\s+(\S+)", uring, re.MULTILINE)[1]
    manifest += [
        f"liburing = {json.dumps(uring_version)}",
        "ryu_revision = "
        + json.dumps(
            re.search(
                r"固定 commit：`([0-9a-f]+)`", (sysroot / "vendor/ryu/README.scoop.md").read_text()
            )[1]
        ),
    ]
    for target in targets:
        manifest += ["", f"[targets.{target}]"]
        manifest += [
            f"{key} = {json.dumps(value)}" for key, value in release["targets"][target].items()
        ]
    for cone in cones:
        manifest += [
            "",
            "[[cones]]",
            f"coordinate = {json.dumps(':'.join(cone[key] for key in ('group', 'name', 'version')))}",
        ]
    (share / "toolchain.toml").write_text("\n".join(manifest) + "\n")


def archive(directory, destination, *, arcname="."):
    staged = destination.with_suffix(destination.suffix + ".new")
    try:
        with tarfile.open(staged, "w:xz", preset=6) as output:
            output.add(directory, arcname=arcname)
        staged.replace(destination)
    finally:
        staged.unlink(missing_ok=True)
    print(f"Created {destination}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version")
    parser.add_argument("--revision")
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "tmp/dist")
    parser.add_argument(
        "--llvm-source",
        type=Path,
        help="Use an existing LLVM source directory instead of extracting the fixed archive",
    )
    parser.add_argument(
        "--llvm-archive",
        type=Path,
        help="Use a local LLVM source archive instead of downloading the fixed release",
    )
    parser.add_argument("--llvm-prefix", type=Path, help="Reuse an installed matching LLVM version")
    parser.add_argument(
        "--prepared-sysroot",
        type=Path,
        help="Use an explicitly prepared sysroot without rebuilding native backends, Cones, or runtime",
    )
    parser.add_argument(
        "--debugger-prefix",
        type=Path,
        help="Reuse a matching Python-enabled Scoop LLDB installation",
    )
    parser.add_argument(
        "--debugger-python", default=sys.executable, help="Python used to build and bundle LLDB"
    )
    parser.add_argument(
        "--python-license",
        type=Path,
        help="Explicit Python license when absent from its installation",
    )
    parser.add_argument("--gnu-cc", default="gcc")
    parser.add_argument("--musl-cc", default="musl-gcc")
    parser.add_argument("--unwind-cc", default="clang")
    parser.add_argument("--unwind-cxx", default="clang++")
    parser.add_argument("--musl-include", type=Path, help="Linux musl development headers")
    package_windows.arguments(parser)
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    args.host = HOSTS.get((platform.system(), platform.machine()))
    if args.host is None or args.jobs < 1:
        parser.error("a supported build host and positive --jobs are required")
    if "linux" in args.host and args.musl_include is None:
        parser.error("Linux packaging requires --musl-include")
    windows = "windows" in args.host
    if windows and (args.mingw_msvcrt is None or args.mingw_ucrt is None):
        parser.error("Windows packaging requires --mingw-msvcrt and --mingw-ucrt")
    release = tomllib.loads(RELEASE_CONFIG.read_text())
    args.version = args.version or release["version"]
    if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", args.version) is None:
        parser.error("--version must be usable as an archive name")
    if args.revision is None:
        args.revision = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
        if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT):
            args.revision += "+modified"
    if not args.revision or "\n" in args.revision:
        parser.error("--revision must be nonempty and on one line")
    args.work_dir = (args.work_dir or ROOT / "tmp/package" / args.host).resolve()
    args.output_dir = args.output_dir.resolve()
    if args.prepared_sysroot is not None:
        args.prepared_sysroot = args.prepared_sysroot.resolve(strict=True)
    sysroot = args.prepared_sysroot or ROOT / "sysroot"
    scratch = args.work_dir / "scratch"
    scratch.mkdir(parents=True, exist_ok=True)
    environment = dict(
        os.environ,
        TMPDIR=str(scratch),
        TMP=str(scratch),
        TEMP=str(scratch),
        SCOOP_RELEASE_VERSION=args.version,
        SCOOP_SOURCE_REVISION=args.revision,
        CARGO_TARGET_DIR=str(args.work_dir / "cargo"),
    )
    if args.host.endswith("darwin"):
        environment["MACOSX_DEPLOYMENT_TARGET"] = release["targets"][args.host]["minimum_os"]
    if windows:
        package_windows.host_environment(args, environment)
    args.llvm_source, args.llvm_archive = package_llvm.prepare(
        args.work_dir, args.llvm_source, args.llvm_archive
    )
    extension = package_tooling.build(args.work_dir, environment)
    if args.llvm_prefix is None:
        args.llvm_prefix = args.work_dir / "llvm/installed"
        run_script(
            "build_llvm.py",
            [
                "--source",
                args.llvm_source,
                "--build-dir",
                args.work_dir / "llvm",
                "--prefix",
                args.llvm_prefix,
                "--jobs",
                args.jobs,
                *(
                    [
                        "--cc",
                        args.mingw_ucrt / "bin/clang.exe",
                        "--cxx",
                        args.mingw_ucrt / "bin/clang++.exe",
                    ]
                    if windows
                    else []
                ),
            ],
            environment,
        )
    args.llvm_prefix = args.llvm_prefix.resolve()
    suffix = ".exe" if windows else ""
    if windows:
        environment["PATH"] = str(args.llvm_prefix / "bin") + os.pathsep + environment["PATH"]
    actual_llvm = subprocess.check_output(
        [str(args.llvm_prefix / "bin" / ("llvm-config" + suffix)), "--version"],
        text=True,
        env=environment,
    ).strip()
    if actual_llvm != release["llvm"]:
        parser.error(f"release requires LLVM {release['llvm']}, found {actual_llvm}")
    if args.debugger_prefix is None:
        args.debugger_prefix = args.work_dir / "lldb/installed"
        run_script(
            "build_lldb.py",
            [
                "--source",
                args.llvm_source,
                "--llvm-prefix",
                args.llvm_prefix,
                "--build-dir",
                args.work_dir / "lldb",
                "--prefix",
                args.debugger_prefix,
                "--python",
                args.debugger_python,
                *(
                    [
                        "--cc",
                        args.mingw_ucrt / "bin/clang.exe",
                        "--cxx",
                        args.mingw_ucrt / "bin/clang++.exe",
                    ]
                    if windows
                    else []
                ),
                "--jobs",
                args.jobs,
            ],
            environment,
        )
    args.debugger_prefix = args.debugger_prefix.resolve()
    environment["LLVM_SYS_221_PREFIX"] = str(args.llvm_prefix)
    host_target = "x86_64-w64-windows-llvm-mingw-ucrt" if windows else args.host
    flags = ["-C", "target-cpu=" + release["targets"][host_target]["cpu"]]
    if not windows:
        flags += ["-C", f"link-arg=-Wl,-rpath,{args.llvm_prefix / 'lib'}"]
    environment["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
    subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "--locked",
            "-p",
            "scoop",
            "-p",
            "scoopc",
            "-p",
            "scoop-linker",
            "-p",
            "scoop-lsp",
            "-p",
            "scoop-pm",
            "--bins",
            "-j",
            str(args.jobs),
        ],
        cwd=ROOT,
        env=environment,
        check=True,
    )
    cargo_output = args.work_dir / "cargo"
    if windows:
        cargo_output /= args.host
    binaries = [cargo_output / "release" / (name + suffix) for name in TOOLS]
    example_environment = environment.copy()
    example_environment.pop("CARGO_ENCODED_RUSTFLAGS", None)
    example = build_scoop_example.build(args.work_dir / "scoop-example", example_environment)
    build_scoop_example.install(example, sysroot)
    if windows:
        targets = list(package_windows.TARGETS)
    else:
        targets = (
            [args.host]
            if args.host.endswith("darwin")
            else [args.host, "x86_64-unknown-linux-musl"]
        )
    if args.prepared_sysroot is None:
        prepare_targets(args, targets, binaries, environment)
    documentation = package_documentation.generate(
        sysroot,
        targets,
        binaries,
        args.work_dir,
        environment,
        native_sysroots={
            target: package_windows.mingw_prefix(args, target)
            for target in targets
            if windows and package_windows.mingw_prefix(args, target) is not None
        },
    )
    args.output_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="assemble-", dir=args.work_dir) as temporary:
        root = Path(temporary) / f"scoop-{args.version}-{args.host}"
        assemble(root, targets, binaries, args, release, extension)
        package_documentation.install(root / "docs", documentation)
        sources = Path(temporary) / f"scoop-{args.version}-llvm-sources"
        vendor = sources / "vendor/llvm-project" / VERSION
        shutil.copytree(
            ROOT / "sysroot/vendor/llvm-project" / VERSION / "patches", vendor / "patches"
        )
        shutil.copy2(
            ROOT / "scripts/patches/lldb-scoop-source-names.patch",
            vendor / "patches/lldb-scoop-source-names.patch",
        )
        shutil.copy2(
            ROOT / "sysroot/vendor/llvm-project" / VERSION / "README.md", vendor / "README.md"
        )
        shutil.copy2(args.llvm_archive, vendor / f"llvm-project-{VERSION}.src.tar.xz")
        shutil.copy2(args.llvm_source / "llvm/LICENSE.TXT", vendor / "LICENSE.TXT")
        archive(sources, args.output_dir / f"{sources.name}.tar.xz", arcname=sources.name)
        archive(root, args.output_dir / f"{root.name}.tar.xz")


if __name__ == "__main__":
    main()
