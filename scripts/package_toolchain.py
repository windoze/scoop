#!/usr/bin/env python3
"""Build a complete Scoop archive, including sysroot, VS Code tooling, and programming skill."""

import argparse
import concurrent.futures
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
from urllib.parse import quote

import build_scoop_api
import build_scoop_bundle
import build_scoop_example
import build_scoop_pgo
import package_debugger
import package_licenses
import package_llvm
import package_manifest
import package_native
import package_tooling
import package_windows
from build_llvm import VERSION
from build_rust_text import LIBRARY as RUST_TEXT_LIBRARY
from build_skia import LIBRARY as SKIA_LIBRARY
from build_skia import TARGETS as SKIA_TARGETS
from build_support import RELEASE_CONFIG, ROOT
from compression_backends import PACKAGES as COMPRESSION_PACKAGES
from download_support import download_directory
from prepare_gtk import LINK_INPUTS as GTK_LINK_INPUTS
from prepare_gtk import TARGET as GTK_TARGET
from prepare_sysroot import cone_order
from prepare_tray import LINK_INPUTS as TRAY_LINK_INPUTS
from prepare_tray import TARGET as TRAY_TARGET
from target_catalog import TARGETS, current_host, host_definition, native_target_for_host

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
        facts = TARGETS[target]
        native = []
        openssl = [
            "--target",
            target,
            "--build-dir",
            args.work_dir / "openssl" / target,
            "--jobs",
            args.jobs,
        ]
        if facts["os"] == "linux":
            cc = args.musl_cc if facts["env"] == "musl" else args.gnu_cc
            native = ["--cc", cc]
            openssl += native
        elif facts["os"] == "windows":
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
        if target in SKIA_TARGETS:
            skia_native = (
                ["--cc", args.llvm_prefix / "bin/clang"] if facts["os"] == "linux" else native
            )
            run_script(
                "build_skia.py",
                [
                    "--target",
                    target,
                    "--build-dir",
                    args.work_dir / "skia" / target,
                    "--gn",
                    args.gn,
                    "--jobs",
                    args.jobs,
                    *skia_native,
                ],
                environment,
            )
        if target == GTK_TARGET:
            run_script(
                "prepare_gtk.py",
                [
                    "--development-prefix",
                    args.gtk_prefix,
                    "--build-dir",
                    args.work_dir / "gtk" / target,
                ],
                environment,
            )
        if target == TRAY_TARGET:
            run_script(
                "prepare_tray.py",
                [
                    "--development-prefix",
                    args.tray_prefix,
                    "--build-dir",
                    args.work_dir / "tray" / target,
                ],
                environment,
            )
        if facts["os"] == "linux":
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
            if facts["env"] == "musl":
                unwind += ["--musl-include", args.musl_include]
            run_script("build_llvm_unwind.py", unwind, environment)
            native += ["--unwind-prefix", sysroot / "lib" / target / "unwind"]
        elif facts["os"] == "windows":
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
        profile_archive = args.work_dir / "profile-runtime" / target / "profile.a"
        profile_cc = (
            package_windows.compiler(args, target)
            if facts["os"] == "windows"
            else args.unwind_cc
            if facts["os"] == "linux"
            else "clang"
        )
        profile_options = ["--cc", profile_cc]
        if facts["env"] == "musl":
            profile_options += ["--musl-include", args.musl_include]
        run_script(
            "build_profile_runtime.py",
            [
                "--target",
                target,
                "--source",
                args.llvm_source,
                "--build-dir",
                profile_archive.parent,
                "--output",
                profile_archive,
                "--jobs",
                args.jobs,
                *profile_options,
            ],
            environment,
        )
        run_script(
            "prepare_sysroot.py",
            [
                "--target",
                target,
                "--scoop",
                binaries[0],
                "--sysroot",
                sysroot,
                "--work-dir",
                args.work_dir / "sysroot-profiling" / target,
                "--cache-dir",
                args.work_dir / "cache",
                "--jobs",
                args.jobs,
                "--profiling",
                "--profile-runtime",
                profile_archive,
                *native,
            ],
            environment,
        )


def relocate_markdown_links(root, revision):
    """Keep repository documentation references usable outside the checkout."""
    repository = (
        "https://github.com/scoop-lang/scoop/blob/"
        + quote(revision.removesuffix("+modified"), safe="")
        + "/"
    )
    for path in (root / "src").rglob("*.md"):
        prefix = "../" * len(path.relative_to(root).parts)
        text = path.read_text(encoding="utf-8")
        updated = re.sub(
            r"(?<=\]\()" + re.escape(prefix) + r"(?=(?:docs|scripts|tests)/)", repository, text
        )
        vendor_prefix = "../" * (len(path.relative_to(root).parts) - 1)
        updated = re.sub(
            r"(?<=\]\()" + re.escape(vendor_prefix) + r"(?=vendor/)",
            repository + "sysroot/",
            updated,
        )
        if updated != text:
            path.write_text(updated, encoding="utf-8")


def stage_binaries(root, binaries, host_system):
    (root / "bin").mkdir(parents=True, exist_ok=True)
    for binary in binaries:
        name = "sco.exe" if host_system == "Windows" and binary.name == "scoop.exe" else binary.name
        shutil.copy2(binary, root / "bin" / name)


def assemble_llvm(root, args, host_system):
    """Stage selected LLVM tools and the debugger in the host command component."""
    native = package_llvm.stage(root, args.llvm_prefix, host_system)
    debugger, version, license_file, search = package_debugger.stage(
        root,
        args.llvm_prefix,
        args.debugger_prefix,
        args.debugger_python,
        args.host,
        args.python_license,
        windows_compiler=args.mingw_ucrt / "bin/clang.exe" if host_system == "Windows" else None,
    )
    native.update(debugger)
    return native, version, license_file, search


def stage_commands(root, args):
    sysroot = args.prepared_sysroot or ROOT / "sysroot"
    shutil.copytree(
        sysroot / "commands", root / "commands", ignore=shutil.ignore_patterns(".gitkeep")
    )
    suffix = ".exe" if host_definition(args.host)["system"] == "Windows" else ""
    return {
        (sysroot / "commands" / (name + suffix)).resolve(): root / "commands" / (name + suffix)
        for name in ("scoop-example", "scoop-bundle", "scoop-api", "scoop-pgo")
    }


def assemble_sources(root, args):
    """Stage Scoop Cones, native glue, headers and examples for source navigation."""
    sysroot = args.prepared_sysroot or ROOT / "sysroot"
    for name in ("src", "include"):
        shutil.copytree(
            sysroot / name,
            root / name,
            ignore=shutil.ignore_patterns(
                "target", "Cone.lock", "*.slib", "__pycache__", ".DS_Store"
            ),
        )
    relocate_markdown_links(root, args.revision)
    shutil.copytree(
        sysroot / "native/text_backend",
        root / "native/text_backend",
        ignore=shutil.ignore_patterns("target"),
    )
    shutil.copytree(sysroot / "share/scoop/examples", root / "share/scoop/examples")


def assemble_vendor(root, args):
    """Stage selected upstream sources and build patches, independently of Cones."""
    sysroot = args.prepared_sysroot or ROOT / "sysroot"
    for vendor in (sysroot / "vendor").iterdir():
        if not vendor.is_dir():
            continue
        destination = root / "vendor" / vendor.name
        if vendor.name == "llvm-project":
            version = vendor / VERSION
            shutil.copytree(version / "patches", destination / VERSION / "patches")
            shutil.copy2(version / "README.md", destination / VERSION / "README.md")
            shutil.copy2(
                ROOT / "scripts/patches/lldb-scoop-source-names.patch",
                destination / VERSION / "patches/lldb-scoop-source-names.patch",
            )
            shutil.copy2(
                args.llvm_archive, destination / VERSION / f"llvm-project-{VERSION}.src.tar.xz"
            )
            shutil.copy2(
                args.llvm_source / "llvm/LICENSE.TXT", destination / VERSION / "LICENSE.TXT"
            )
        else:
            shutil.copytree(vendor, destination)
    package_licenses.collect_vendor(root, sysroot)


GUI_ROOT_CONES = ("scoop.window", "scoop.graphics")


def split_gui_cones(sysroot, order):
    """Split one target's Cone order into base Cones and GUI Cones.

    GUI Cones are the window and graphics roots and every Cone depending on them.
    """
    manifests = {
        directory: tomllib.loads(
            (sysroot / "src" / directory / "Cone.toml").read_text(encoding="utf-8")
        )
        for directory in order
    }
    directories = {
        f"{manifest['cone']['group']}:{manifest['cone']['name']}": directory
        for directory, manifest in manifests.items()
    }
    gui = set()
    for directory in order:
        dependencies = manifests[directory].get("dependencies", {})
        if directory in GUI_ROOT_CONES or any(
            directories.get(name) in gui for name in dependencies
        ):
            gui.add(directory)
    return (
        [directory for directory in order if directory not in gui],
        [directory for directory in order if directory in gui],
    )


def assemble_base_libraries(root, target, args, cones):
    """Stage one target's base system libraries, native inputs and runtimes."""
    sysroot = args.prepared_sysroot or ROOT / "sysroot"
    source = sysroot / "lib" / target
    destination = root / "lib" / target
    destination.mkdir(parents=True)
    for cone in cones:
        shutil.copy2(source / f"{cone}.slib", destination / f"{cone}.slib")
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
        (destination / variant).mkdir(parents=True)
        archive = "runtime.lib" if TARGETS[target]["os"] == "windows" else "runtime.a"
        for name in ("index.cbor", archive):
            shutil.copy2(source / variant / name, destination / variant / name)
    for component, libraries in (
        ("openssl", ("libcrypto.a", "libssl.a")),
        ("nghttp2", ("libnghttp2.a",)),
        ("unwind", ("libunwind.a",)),
    ):
        if component == "unwind" and TARGETS[target]["os"] == "darwin":
            continue
        (destination / component / "lib").mkdir(parents=True)
        for name in libraries:
            shutil.copy2(source / component / "lib" / name, destination / component / "lib" / name)
    package_licenses.collect_base_libraries(root, sysroot, target)


def assemble_profiling_libraries(root, target, args, cones):
    """Stage both full-template library sets without duplicating native resources."""
    sysroot = args.prepared_sysroot or ROOT / "sysroot"
    source = sysroot / "lib" / target / "profiling"
    destination = root / "lib" / target / "profiling"
    for variant in ("instrumented", "release"):
        (destination / variant).mkdir(parents=True)
        for cone in cones:
            names = [f"{cone}.slib"]
            if variant == "instrumented":
                names.append(f"{cone}.slib.profile.json")
            for name in names:
                shutil.copy2(source / variant / name, destination / variant / name)
    archive = "runtime.lib" if TARGETS[target]["os"] == "windows" else "runtime.a"
    for variant in (
        "runtime",
        "runtime-stress",
        "runtime-startup-stats",
        "runtime-stress-startup-stats",
    ):
        (destination / variant).mkdir()
        for name in ("index.cbor", archive, "profile.a"):
            shutil.copy2(source / variant / name, destination / variant / name)
    package_licenses.collect_profiling(root, args.llvm_source, target)


def assemble_gui_libraries(root, target, args, cones):
    """Stage one target's GUI system libraries and their native inputs."""
    sysroot = args.prepared_sysroot or ROOT / "sysroot"
    source = sysroot / "lib" / target
    destination = root / "lib" / target
    destination.mkdir(parents=True)
    for cone in cones:
        shutil.copy2(source / f"{cone}.slib", destination / f"{cone}.slib")
    if target in SKIA_TARGETS:
        shutil.copy2(source / SKIA_LIBRARY, destination / SKIA_LIBRARY)
    if target == GTK_TARGET:
        for library in GTK_LINK_INPUTS:
            shutil.copy2(source / library, destination / library)
    if target == TRAY_TARGET:
        for library in TRAY_LINK_INPUTS:
            shutil.copy2(source / library, destination / library)
    package_licenses.collect_gui_libraries(root, sysroot, target)


def assemble_bin(root, targets, args, release, extension, orders, python_version, native):
    """Stage the host command package, installation manifest and shared documents."""
    sysroot = args.prepared_sysroot or ROOT / "sysroot"
    host_system = host_definition(args.host)["system"]
    (root / "VERSION").write_text(
        f"{args.version}\nrevision: {args.revision}\nhost: {args.host}\n", encoding="utf-8"
    )
    (root / "tooling").mkdir()
    shutil.copy2(extension, root / "tooling" / extension.name)
    shutil.copytree(ROOT / "tooling/scoop-programming", root / "tooling/scoop-programming")
    cone_manifests = [
        tomllib.loads(path.read_text(encoding="utf-8"))
        for path in sorted((sysroot / "src").glob("*/Cone.toml"))
    ]
    share = root / "share/scoop"
    share.mkdir(parents=True, exist_ok=True)
    for name in ("README.md", "RELEASE-NOTES.md"):
        shutil.copy2(ROOT / "scripts/distribution" / name, share / name)
    installer = "post-install.ps1" if host_system == "Windows" else "post-install.sh"
    shutil.copy2(ROOT / "scripts/distribution" / installer, root / installer)
    if installer.endswith(".sh"):
        (root / installer).chmod(0o755)
    plan = share / "install"
    plan.mkdir()
    default_target = host_definition(args.host)["default_target"]
    ordered_targets = [default_target, *(target for target in targets if target != default_target)]
    (plan / "targets.list").write_text("\n".join(ordered_targets) + "\n", encoding="utf-8")
    manifest = [
        "schema = 1",
        f"version = {json.dumps(args.version)}",
        f"source_revision = {json.dumps(args.revision)}",
        f"host = {json.dumps(args.host)}",
        f"default_target = {json.dumps(default_target)}",
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
    if host_system == "Windows":
        manifest += [
            f"{name} = {json.dumps(release[name])}" for name in ("rust_windows", "llvm_mingw")
        ]
    if any(target in SKIA_TARGETS for target in targets):
        manifest.append(f"skia = {json.dumps(release['skia'])}")
    manifest += [
        f"{name} = {json.dumps(release[name])}"
        for name in ("text_backend", "encoding_rs", "idna", "icu4x", "unicode", "unicode-width")
    ]
    manifest += [
        f"{package['name']} = {json.dumps(release[package['name']])}"
        for package in COMPRESSION_PACKAGES
    ]
    uring = (sysroot / "vendor/liburing/2.15/liburing.spec").read_text(encoding="utf-8")
    uring_version = re.search(r"^Version:\s+(\S+)", uring, re.MULTILINE)[1]
    manifest += [
        f"liburing = {json.dumps(uring_version)}",
        "ryu_revision = "
        + json.dumps(
            re.search(
                r"固定 commit：`([0-9a-f]+)`",
                (sysroot / "vendor/ryu/README.scoop.md").read_text(encoding="utf-8"),
            )[1]
        ),
    ]
    for target in targets:
        manifest += ["", f"[targets.{target}]"]
        manifest += [
            f"{key} = {json.dumps(value)}" for key, value in release["targets"][target].items()
        ]
        base_cones, gui_cones = split_gui_cones(sysroot, orders[target])
        manifest.append(f"cones = {json.dumps(base_cones)}")
        manifest.append(f"gui_cones = {json.dumps(gui_cones)}")
        (plan / f"{target}.list").write_text("\n".join(base_cones) + "\n", encoding="utf-8")
        if gui_cones:
            (plan / f"{target}.gui.list").write_text("\n".join(gui_cones) + "\n", encoding="utf-8")
    for cone_manifest in cone_manifests:
        cone = cone_manifest["cone"]
        manifest += [
            "",
            "[[cones]]",
            f"coordinate = {json.dumps(':'.join(cone[key] for key in ('group', 'name', 'version')))}",
        ]
    (share / "toolchain.toml").write_text("\n".join(manifest) + "\n", encoding="utf-8")
    package_licenses.collect_bin(root, ROOT, args.host)


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
        "--vscode-extension", type=Path, help="Reuse an already built matching VS Code extension"
    )
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
    parser.add_argument(
        "--download-dir", type=Path, help="Reuse upstream downloads (SCOOP_DOWNLOAD_DIR)"
    )
    parser.add_argument("--llvm-prefix", type=Path, help="Reuse an installed matching LLVM version")
    parser.add_argument("--gn", default="gn", help="GN executable used to prepare Skia resources")
    parser.add_argument(
        "--gtk-prefix",
        type=Path,
        default=Path("/usr"),
        help="Linux GTK/libadwaita development prefix used by pkg-config",
    )
    parser.add_argument(
        "--tray-prefix",
        type=Path,
        default=Path("/usr"),
        help="Linux libdbusmenu-glib development prefix used by scoop.tray",
    )
    parser.add_argument(
        "--prepared-sysroot",
        type=Path,
        help="Use a prepared sysroot (native backends, runtime and system slibs) as-is",
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
    parser.add_argument(
        "--gnu-cc", help="GNU target C compiler; defaults to the prepared LLVM Clang"
    )
    parser.add_argument("--musl-cc", default="musl-gcc")
    parser.add_argument("--unwind-cc", default="clang")
    parser.add_argument("--unwind-cxx", default="clang++")
    parser.add_argument("--musl-include", type=Path, help="Linux musl development headers")
    package_windows.arguments(parser)
    parser.add_argument("--jobs", type=int, default=min(os.cpu_count() or 1, 6))
    args = parser.parse_args()
    host_config = current_host(platform.system(), platform.machine())
    args.host = host_config["rust_target"] if host_config else None
    if args.host is None or args.jobs < 1:
        parser.error("a supported build host and positive --jobs are required")
    host_system = host_config["system"]
    if host_system == "Linux" and args.musl_include is None:
        parser.error("Linux packaging requires --musl-include")
    windows = host_system == "Windows"
    if windows and (args.mingw_msvcrt is None or args.mingw_ucrt is None):
        parser.error("Windows packaging requires --mingw-msvcrt and --mingw-ucrt")
    release = tomllib.loads(RELEASE_CONFIG.read_text(encoding="utf-8"))
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
    args.download_dir = download_directory(args.download_dir)
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
    if host_system == "Darwin":
        host_target, _ = native_target_for_host(args.host)
        environment["MACOSX_DEPLOYMENT_TARGET"] = release["targets"][host_target]["minimum_os"]
    if windows:
        package_windows.host_environment(args, environment)
    args.llvm_source, args.llvm_archive = package_llvm.prepare(
        args.work_dir, args.llvm_source, args.llvm_archive, args.download_dir
    )
    extension = args.vscode_extension or package_tooling.build(args.work_dir, environment)
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
    if host_system == "Linux" and args.gnu_cc is None:
        args.gnu_cc = str(args.llvm_prefix / "bin/clang")
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
    flags = []
    if windows:
        host_target, _ = native_target_for_host(args.host)
        flags = [
            "-C",
            "target-cpu=" + release["targets"][host_target]["cpu"],
        ]
    else:
        flags = ["-C", f"link-arg=-Wl,-rpath,{args.llvm_prefix / 'lib'}"]
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
            "-p",
            "scoop-api",
            "-p",
            "scoop-pgo",
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
    build_scoop_api.install(cargo_output / "release" / ("scoop-api" + suffix), sysroot)
    build_scoop_pgo.install(cargo_output / "release" / ("scoop-pgo" + suffix), sysroot)
    bundle = build_scoop_bundle.build(args.work_dir / "scoop-bundle", example_environment)
    build_scoop_bundle.install(bundle, sysroot)
    targets = list(host_definition(args.host)["distribution_targets"])
    if args.prepared_sysroot is None:
        prepare_targets(args, targets, binaries, environment)
    orders = {}
    for target in targets:
        selection = args.work_dir / "cones" / target
        selection.mkdir(parents=True, exist_ok=True)
        orders[target] = cone_order(sysroot, selection, target)
    output = args.output_dir / f"scoop-{args.version}"
    output.mkdir(parents=True, exist_ok=True)
    packages = []
    pending = []

    def publish(component, directory, *, arcname="."):
        name = f"scoop-{args.version}-{component}.tar.xz"
        pending.append((name, archiver.submit(archive, directory, output / name, arcname=arcname)))

    with (
        tempfile.TemporaryDirectory(prefix="assemble-", dir=args.work_dir) as temporary,
        concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as archiver,
    ):
        base = Path(temporary)
        bin_root = base / "bin"
        stage_binaries(bin_root, binaries, host_system)
        debugger_files, python_version, python_license, debugger_search = assemble_llvm(
            bin_root, args, host_system
        )
        debugger_files.update(stage_commands(bin_root, args))
        if host_system != "Darwin":
            package_llvm.stage_unwind_headers(bin_root, args.llvm_source)
        if windows:
            package_windows.stage_shared_headers(bin_root, args)
        if host_system == "Windows":
            native = package_native.windows(
                bin_root,
                bin_root,
                binaries,
                args.llvm_prefix / "bin",
                debugger_files,
                search_directories=debugger_search,
            )
        elif host_system == "Darwin":
            native = package_native.darwin(
                bin_root, bin_root, binaries, args.llvm_prefix / "lib", debugger_files
            )
        else:
            native = package_native.linux(
                bin_root,
                bin_root,
                binaries,
                args.llvm_prefix / "lib",
                debugger_files,
                search_directories=debugger_search,
            )
        package_licenses.collect_llvm(bin_root, args.llvm_source, args.host, native, python_license)
        assemble_bin(bin_root, targets, args, release, extension, orders, python_version, native)
        if windows:
            package_llvm.share_windows_aliases(bin_root, package_debugger.LLVM_TOOLS)
        sources_root = base / "sources"
        assemble_sources(sources_root, args)
        publish(f"bin-{args.host}", bin_root)
        publish("sources", sources_root)
        for target in targets:
            base_cones, gui_cones = split_gui_cones(sysroot, orders[target])
            lib_root = base / f"lib-base-{target}"
            assemble_base_libraries(lib_root, target, args, base_cones)
            publish(f"lib-base-{target}", lib_root)
            if gui_cones:
                gui_root = base / f"lib-gui-{target}"
                assemble_gui_libraries(gui_root, target, args, gui_cones)
                publish(f"lib-gui-{target}", gui_root)
            profiling_root = base / f"lib-profiling-{target}"
            assemble_profiling_libraries(profiling_root, target, args, orders[target])
            publish(f"lib-profiling-{target}", profiling_root)
            if windows and package_windows.mingw_prefix(args, target) is not None:
                native_root = base / f"native-{target}"
                package_windows.stage_target_sdk(native_root, target, args)
                publish(f"native-{target}", native_root)
        vendor_root = base / "vendor"
        assemble_vendor(vendor_root, args)
        publish("vendor", vendor_root)
        for name, task in pending:
            task.result()
            packages.append(name)
    listing = output / f"packages-{args.host}.list"
    staged = listing.with_suffix(".list.new")
    staged.write_text("\n".join(packages) + "\n", encoding="utf-8")
    staged.replace(listing)
    package_manifest.produce(
        output, args.version, args.revision, args.host, targets, packages, release
    )


if __name__ == "__main__":
    main()
