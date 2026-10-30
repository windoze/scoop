"""Command-line discovery, environment setup, and acceptance reporting."""

import argparse
import dataclasses
import errno
import fnmatch
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from collections import Counter
from pathlib import Path

from scripts.target_catalog import TARGETS, WINDOWS_TARGETS, current_host

from .cores import (
    prepare_class_core,
    prepare_inheritance_core,
    prepare_member_defaults_core,
    prepare_test_core,
)
from .discovery import discover
from .model import ConfigurationError, EnvironmentError
from .suite import run_suite


def arguments(argv):
    parser = argparse.ArgumentParser(description="Run declared Scoop process fixtures")
    parser.add_argument("--all", action="store_true", help="complete suite, without selection")
    parser.add_argument("--suite", type=Path, help="development discovery root")
    parser.add_argument("--filter", action="append", default=[], help="name/tag glob")
    parser.add_argument("--list", action="store_true")
    parser.add_argument(
        "--compile-only", action="store_true", help="skip fixtures requiring target execution"
    )
    parser.add_argument("--update-snapshots", action="store_true")
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--cache-dir", type=Path, help="shared Cone cache, independent of work-dir")
    parser.add_argument(
        "--sysroot", type=Path, help="prepared system libraries and native resources"
    )
    parser.add_argument("--keep", action="store_true")
    parser.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 1))
    parser.add_argument("--scoop", type=Path)
    parser.add_argument("--scoopc", type=Path)
    parser.add_argument("--scoop-link", type=Path)
    parser.add_argument("--scoop-lsp", type=Path)
    parser.add_argument("--scoop-pm", type=Path)
    parser.add_argument("--terminal-host", type=Path, help="prebuilt native PTY test host")
    parser.add_argument(
        "--target",
        choices=list(TARGETS),
    )
    parser.add_argument("--cc", type=Path, help="native companion compiler for the selected target")
    parser.add_argument("--ar", type=Path, help="native companion archiver")
    parser.add_argument(
        "--native-sysroot", type=Path, help="selected target's C development resources"
    )
    parser.add_argument("--unwind-prefix", type=Path, help="selected target's LLVM unwind prefix")
    parser.add_argument("--linker", type=Path, help="final executable linker")
    parser.add_argument("--llc", type=Path, help="LLVM 22.1 compiler for IR companions")
    args = parser.parse_args(argv)
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    if args.all and (args.suite or args.filter or args.update_snapshots):
        parser.error("--all cannot be combined with suite/filter/snapshot updates")
    if args.update_snapshots and not (args.suite or args.filter):
        parser.error("snapshot updates require an explicit selection")
    return args


def tool_output(argv, *, input=None):
    result = subprocess.run(argv, input=input, capture_output=True, check=False)
    if result.returncode:
        raise EnvironmentError(f"tool discovery {argv!r} failed: {result.stderr!r}")
    return result.stdout.decode().strip()


def selected_target(args):
    if args.target:
        return args.target
    host = current_host(platform.system(), platform.machine())
    if host:
        return host["default_target"]
    raise EnvironmentError("unknown fixture host; select the compilation target with --target")


def can_execute(target):
    host = current_host(platform.system(), platform.machine())
    if host is None:
        return False
    default = TARGETS[host["default_target"]]
    selected = TARGETS[target]
    if all(selected[key] == default[key] for key in ("os", "arch")):
        return True
    if (default["os"], default["arch"], selected["os"], selected["arch"]) == (
        "darwin",
        "aarch64",
        "darwin",
        "x86_64",
    ):
        try:
            return (
                subprocess.run(
                    ["/usr/bin/arch", "-x86_64", "/usr/bin/true"],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    check=False,
                ).returncode
                == 0
            )
        except OSError:
            return False
    return False


def environment(repo, work, fixtures, args):
    target = selected_target(args)
    applicable = [
        fixture
        for fixture in fixtures
        if (not fixture.data.get("targets") or target in fixture.data["targets"])
        and not (args.compile_only and fixture.data.get("requires_target_execution", True))
    ]
    if any(f.data.get("requires_target_execution", True) for f in applicable) and not can_execute(
        target
    ):
        host = (platform.system(), platform.machine())
        raise EnvironmentError(f"target {target!r} cannot execute on fixture host {host}")
    if os.name == "nt":
        import ctypes

        missing_file = ctypes.FormatError(2).strip()
        directory_error = 5
        output_directory = ctypes.FormatError(directory_error).strip()
    else:
        missing_file = os.strerror(errno.ENOENT)
        directory_error = errno.EISDIR
        output_directory = os.strerror(directory_error)
    cache = Path(
        args.cache_dir or os.environ.get("SCOOP_CACHE_DIR") or repo / "target/fixture-cache"
    ).resolve()
    sysroot = Path(args.sysroot or os.environ.get("SCOOP_SYSROOT") or repo / "sysroot").resolve()
    common = {
        "repo": str(repo),
        "runtime": str(repo / "runtime"),
        "cache": str(cache),
        "sysroot": str(sysroot),
        "test_core": str(work / "test-core"),
        "class_core": str(work / "class-core"),
        "inheritance_core": str(work / "inheritance-core"),
        "member_defaults_core": str(work / "member-defaults-core"),
        "target": target,
        "llvm_target": TARGETS[target]["llvm_triple"],
        "target_profile": TARGETS[target]["profile"],
        "symbol_prefix": "_" if TARGETS[target]["os"] == "darwin" else "",
        "errno_eloop": errno.ELOOP,
        "file_not_found_error": f"{missing_file} (os error 2)",
        "directory_write_error": f"{output_directory} (os error {directory_error})",
        "python": sys.executable,
        "compile_args": ["--target", target],
        "cc_args": [],
    }
    if args.cc:
        common["compile_args"] += ["--cc", str(args.cc.resolve())]
    if args.native_sysroot:
        common["compile_args"] += ["--native-sysroot", str(args.native_sysroot.resolve())]
    common["link_args"] = list(common["compile_args"])
    if TARGETS[target]["os"] != "darwin":
        unwind = (
            args.unwind_prefix.resolve()
            if args.unwind_prefix
            else sysroot / "lib" / target / "unwind"
        )
        common["link_args"] += ["--unwind-prefix", str(unwind)]
    if args.linker:
        common["link_args"] += ["--linker", str(args.linker.resolve())]
    needed = set().union(*(set(fixture.data.get("tools", ["scoop"])) for fixture in applicable))
    for name in ("scoop", "scoopc", "scoop-link", "scoop-lsp", "scoop-pm", "terminal-host"):
        configured = getattr(args, name.replace("-", "_")) or os.environ.get(
            "SCOOP_TEST_PAIRED_" + name.upper().replace("-", "_")
        )
        binary = (
            Path(configured)
            if configured
            else repo / "target/debug" / (name + (".exe" if os.name == "nt" else ""))
        )
        common[name] = str(binary.resolve())
        if name in needed and (not binary.is_file() or not os.access(binary, os.X_OK)):
            raise EnvironmentError(f"required executable is missing: {binary}")
    unknown = needed - {
        "scoop",
        "scoopc",
        "scoop-link",
        "scoop-lsp",
        "scoop-pm",
        "terminal-host",
        "cc",
        "ar",
        "llc",
        "python",
    }
    if unknown:
        raise ConfigurationError(f"unknown required tools: {sorted(unknown)}")
    if "llc" in needed:
        configured = args.llc or os.environ.get("SCOOP_TEST_PAIRED_LLC")
        prefix = os.environ.get("LLVM_SYS_221_PREFIX")
        program = (
            str(configured)
            if configured
            else str(Path(prefix) / "bin" / ("llc.exe" if os.name == "nt" else "llc"))
            if prefix
            else shutil.which("llc-22") or "llc"
        )
        located = shutil.which(program)
        if located is None:
            raise EnvironmentError(f"required LLVM 22.1 tool is missing: {program}")
        if not re.search(r"\bversion 22\.1(?:\.|\b)", tool_output([located, "--version"])):
            raise EnvironmentError(f"IR companions require LLVM 22.1: {located}")
        common["llc"] = located
    if needed & {"cc", "ar"} and TARGETS[target]["os"] == "darwin":
        common.update(
            cc=str(args.cc.resolve())
            if args.cc
            else tool_output(["/usr/bin/xcrun", "--find", "clang"]),
            ar=str(args.ar.resolve())
            if args.ar
            else tool_output(["/usr/bin/xcrun", "--find", "ar"]),
            sdk=str(args.native_sysroot.resolve())
            if args.native_sysroot
            else tool_output(["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"]),
            deployment=darwin_deployment(repo, target),
        )
        common["cc_args"] = [
            "-target",
            target,
            "-isysroot",
            common["sdk"],
            "-mmacosx-version-min=" + common["deployment"],
        ]
    elif needed & {"cc", "ar"} and target in WINDOWS_TARGETS:
        from .windows_tools import companions

        common.update(companions(target, args, sysroot, needed))
    elif needed & {"cc", "ar"}:
        common["cc_args"] = ["-fPIC", "-pthread"]
        for name, program in {
            "cc": str(args.cc) if args.cc else "musl-gcc" if target.endswith("musl") else "gcc",
            "ar": str(args.ar) if args.ar else "ar",
        }.items():
            if name in needed:
                located = shutil.which(program)
                if located is None:
                    raise EnvironmentError(f"required {target} tool is missing: {program}")
                common[name] = located
        if "cc" in needed:
            macros = tool_output([common["cc"], "-dM", "-E", "-x", "c", "-"], input=b"")
            if "#define __clang__ " in macros:
                common["cc_args"].append("--target=" + TARGETS[target]["llvm_triple"])
            if args.native_sysroot:
                common["cc_args"].append("--sysroot=" + str(args.native_sysroot.resolve()))
    if "llc" in needed and TARGETS[target]["os"] == "darwin":
        if "deployment" not in common:
            common["deployment"] = darwin_deployment(repo, target)
        common["llvm_target"] = (
            TARGETS[target]["llvm_triple"].replace("-darwin", "-macosx") + common["deployment"]
        )
    if any("${test_core}" in json.dumps(fixture.data) for fixture in applicable):
        prepare_test_core(repo, work, sysroot)
    if any("${class_core}" in json.dumps(fixture.data) for fixture in applicable):
        prepare_class_core(repo, work, sysroot)
    if any("${inheritance_core}" in json.dumps(fixture.data) for fixture in applicable):
        prepare_inheritance_core(repo, work, sysroot)
    if any("${member_defaults_core}" in json.dumps(fixture.data) for fixture in applicable):
        prepare_member_defaults_core(repo, work, sysroot)
    cache.mkdir(parents=True, exist_ok=True)
    return common


def darwin_deployment(repo, target):
    configured = os.environ.get("MACOSX_DEPLOYMENT_TARGET")
    if configured is not None:
        return configured
    release = tomllib.loads((repo / "scripts/release.toml").read_text(encoding="utf-8"))
    return release["targets"][target]["minimum_os"]


def main(argv=None):
    args = arguments(argv)
    repo = Path(__file__).resolve().parents[2]
    suite = (args.suite or repo / "tests/fixtures").resolve()
    try:
        target = selected_target(args)
        fixtures = discover(suite, args.update_snapshots, target)
        selected = [
            fixture
            for fixture in fixtures
            if not args.filter
            or any(
                fnmatch.fnmatch(value, pattern)
                for pattern in args.filter
                for value in [fixture.name, *fixture.data.get("tags", [])]
            )
        ]
        if not selected:
            raise ConfigurationError("selection contains no fixtures")
        if args.list:
            for fixture in selected:
                print(f"{fixture.name}\t{fixture.locator}")
            print(
                f"{len(selected)} selected / {len(fixtures)} discovered; all source ownership checked"
            )
            return 0
        work = (
            args.work_dir.resolve()
            if args.work_dir
            else Path(tempfile.mkdtemp(prefix="scoop-fixtures-")).resolve()
        )
        work.mkdir(parents=True, exist_ok=True)
        common = environment(repo, work, selected, args)
    except (OSError, ValueError, EnvironmentError) as error:
        print(f"configuration/environment error: {error}", file=sys.stderr)
        return 2
    print(
        f"fixtures: {len(selected)} selected / {len(fixtures)} discovered; work: {work}",
        flush=True,
    )
    results = run_suite(
        selected, common, work, args.update_snapshots, args.jobs, compile_only=args.compile_only
    )
    counts = Counter(result.status for result in results)
    report = {
        "schema": 1,
        "complete": args.all and not args.compile_only,
        "compile_only": args.compile_only,
        "discovered": len(fixtures),
        "selected": len(selected),
        "unselected": len(fixtures) - len(selected),
        "counts": dict(counts),
        "variants": sum(result.variants for result in results),
        "processes": sum(result.processes for result in results),
        "stage_and_plan_goldens": sum(result.snapshots for result in results),
        "results": [dataclasses.asdict(result) for result in results],
    }
    (work / "report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(
        f"{dict(counts)}; variants={report['variants']}, processes={report['processes']}, goldens={report['stage_and_plan_goldens']}"
    )
    failed = any(
        result.status not in {"passed", "inapplicable", "execution-skipped"} for result in results
    )
    if failed or args.keep or args.work_dir:
        print(f"report and work retained: {work}")
    else:
        shutil.rmtree(work)
    return 130 if counts["interrupted"] else 1 if failed else 0
