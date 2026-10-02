"""Command-line discovery, environment setup, and acceptance reporting."""

import argparse
import dataclasses
import fnmatch
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from pathlib import Path

from .discovery import discover
from .execute import execute
from .model import ConfigurationError, EnvironmentError


def arguments(argv):
    parser = argparse.ArgumentParser(description="Run declared Scoop process fixtures")
    parser.add_argument("--all", action="store_true", help="complete suite, without selection")
    parser.add_argument("--suite", type=Path, help="development discovery root")
    parser.add_argument("--filter", action="append", default=[], help="name/tag glob")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--update-snapshots", action="store_true")
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--keep", action="store_true")
    parser.add_argument("--scoop", type=Path)
    parser.add_argument("--scoopc", type=Path)
    parser.add_argument("--scoop-link", type=Path)
    args = parser.parse_args(argv)
    if args.all and (args.suite or args.filter or args.update_snapshots):
        parser.error("--all cannot be combined with suite/filter/snapshot updates")
    if args.update_snapshots and not (args.suite or args.filter):
        parser.error("snapshot updates require an explicit selection")
    return args


def tool_output(argv):
    result = subprocess.run(argv, capture_output=True, check=False)
    if result.returncode:
        raise EnvironmentError(f"tool discovery {argv!r} failed: {result.stderr!r}")
    return result.stdout.decode().strip()


def environment(repo, work, fixtures, args):
    target = {("Darwin", "arm64"): "aarch64-apple-darwin"}.get(
        (platform.system(), platform.machine())
    )
    if target is None:
        raise EnvironmentError("this acceptance suite requires the supported Darwin/AArch64 target")
    common = {
        "repo": str(repo),
        "runtime": str(repo / "runtime"),
        "cache": str(work / "cache"),
        "sysroot": str(work / "sysroot"),
        "target": target,
        "python": sys.executable,
    }
    needed = set().union(*(set(fixture.data.get("tools", ["scoop"])) for fixture in fixtures))
    for name in ("scoop", "scoopc", "scoop-link"):
        configured = getattr(args, name.replace("-", "_")) or os.environ.get(
            "SCOOP_TEST_PAIRED_" + name.upper().replace("-", "_")
        )
        binary = Path(configured) if configured else repo / "target/debug" / name
        common[name] = str(binary.resolve())
        if name in needed and (not binary.is_file() or not os.access(binary, os.X_OK)):
            raise EnvironmentError(f"required executable is missing: {binary}")
    unknown = needed - {"scoop", "scoopc", "scoop-link", "cc", "ar", "python"}
    if unknown:
        raise ConfigurationError(f"unknown required tools: {sorted(unknown)}")
    if needed & {"cc", "ar"}:
        common.update(
            cc=tool_output(["/usr/bin/xcrun", "--find", "clang"]),
            ar=tool_output(["/usr/bin/xcrun", "--find", "ar"]),
            sdk=tool_output(["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"]),
            deployment=tool_output(["/usr/bin/sw_vers", "-productVersion"]),
        )
    if not (work / "sysroot").exists():
        shutil.copytree(repo / "sysroot", work / "sysroot", symlinks=True)
    (work / "cache").mkdir(exist_ok=True)
    return common


def main(argv=None):
    args = arguments(argv)
    repo = Path(__file__).resolve().parents[2]
    suite = (args.suite or repo / "tests/fixtures").resolve()
    try:
        fixtures = discover(suite, args.update_snapshots)
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
    results = []
    for index, fixture in enumerate(selected):
        if fixture.data.get("targets") and common["target"] not in fixture.data["targets"]:
            from .model import Result

            result = Result(
                fixture.name,
                "inapplicable",
                0,
                message="target is not declared applicable",
            )
        else:
            result = execute(fixture, common, work / "cases" / str(index), args.update_snapshots)
        results.append(result)
        print(f"{result.status}: {result.name} ({result.seconds:.2f}s)", flush=True)
        if result.message:
            print(result.message, flush=True)
    counts = Counter(result.status for result in results)
    report = {
        "schema": 1,
        "complete": args.all,
        "discovered": len(fixtures),
        "selected": len(selected),
        "unselected": len(fixtures) - len(selected),
        "counts": dict(counts),
        "variants": sum(result.variants for result in results),
        "processes": sum(result.processes for result in results),
        "stage_and_plan_goldens": sum(result.snapshots for result in results),
        "results": [dataclasses.asdict(result) for result in results],
    }
    (work / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(
        f"{dict(counts)}; variants={report['variants']}, processes={report['processes']}, goldens={report['stage_and_plan_goldens']}"
    )
    failed = any(result.status not in {"passed", "inapplicable"} for result in results)
    if failed or args.keep or args.work_dir:
        print(f"report and work retained: {work}")
    else:
        shutil.rmtree(work)
    return 1 if failed else 0
