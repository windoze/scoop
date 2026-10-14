"""Build the existing mark graphs with M44 collection-boundary observation."""

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--unwind-prefix", type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    work, tools = args.work.resolve(), args.tools.resolve()
    work.mkdir(parents=True, exist_ok=False)
    report = {
        "revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=repo, text=True
        ).strip(),
        "source_diff": subprocess.check_output(["git", "diff", "HEAD"], cwd=repo, text=True),
        "host": platform.platform(),
        "machine": platform.machine(),
        "cpu_count": os.cpu_count(),
        "target": args.target,
        "profile": "release",
        "world_timing": "test hooks after STOPPING, COLLECTING and RUNNING publication",
        "tools": {},
        "commands": [],
    }
    for name in ("scoop", "scoopc", "scoop-link"):
        report["tools"][name] = {
            "path": str(tools / name),
            "sha256": hashlib.file_digest((tools / name).open("rb"), "sha256").hexdigest(),
        }
    for name, command in (("cc", ["cc", "--version"]), ("llvm", ["llvm-config", "--version"])):
        report["tools"][name] = subprocess.check_output(command, text=True).strip()

    def run(command):
        command = list(map(str, command))
        start = time.perf_counter()
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        report["commands"].append(
            {
                "command": command,
                "seconds": time.perf_counter() - start,
                "returncode": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
            }
        )
        (work / "build.json").write_text(json.dumps(report, indent=2) + "\n")
        result.check_returncode()

    runtime = work / "runtime"
    shutil.copytree(repo / "runtime", runtime)
    shutil.copytree(repo / "sysroot", work / "sysroot")
    header = runtime / "src/thread/testing.h"
    header.write_text("#define SCOOP_THREAD_TESTING\n" + header.read_text())
    run(
        [
            "cc",
            "-std=c11",
            "-D_POSIX_C_SOURCE=200809L",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-DSCOOP_BENCH_PARALLEL_METRICS",
            "-DSCOOP_BENCH_WORLD_TIMING",
            "-I",
            runtime / "include",
            "-c",
            repo / "tests/benchmarks/parallel/mark-phase.c",
            "-o",
            work / "mark-phase.o",
        ]
    )
    run(["ar", "rcs", work / "libm34_mark.a", work / "mark-phase.o"])
    command = [
        tools / "scoop",
        "build",
        repo / "tests/benchmarks/parallel/mark-graphs.scoop",
        "--scoopc",
        tools / "scoopc",
        "--sysroot",
        work / "sysroot",
        "--runtime-root",
        runtime,
        "--cache-dir",
        args.cache.resolve(),
        "--profile",
        "release",
        "--target",
        args.target,
        "--target-dir",
        work / "target",
        "--library-path",
        work,
        "--emit",
        "all",
        "--dump-dir",
        work / "dump",
        "--message-format",
        "json",
        "-o",
        work / "graphs",
    ]
    if args.unwind_prefix:
        command += ["--unwind-prefix", args.unwind_prefix.resolve()]
    run(command)
    print(work / "build.json")


if __name__ == "__main__":
    main()
