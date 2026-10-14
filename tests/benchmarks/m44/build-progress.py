"""Build a fixed progress-observation binary with its matching runtime and tools."""

import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import time
from pathlib import Path


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--sysroot", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--unwind-prefix", type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    work, tools = args.work.resolve(), args.tools.resolve(strict=True)
    work.mkdir(parents=True, exist_ok=False)
    report = {
        "host": platform.platform(),
        "target": args.target,
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "diff": subprocess.check_output(["git", "diff", "HEAD"], text=True),
        "source_runtime": str(args.runtime.resolve()),
        "source_sysroot": str(args.sysroot.resolve()),
        "tools": {
            name: {"path": str(tools / name), "sha256": digest(tools / name)}
            for name in ("scoop", "scoopc", "scoop-link")
        },
        "cc": subprocess.check_output(["cc", "--version"], text=True),
        "llvm": subprocess.check_output(["llvm-config", "--version"], text=True),
        "commands": [],
    }
    runtime = work / "runtime"
    shutil.copytree(args.runtime, runtime)
    shutil.copytree(args.sysroot, work / "sysroot")
    # Legacy allocation included thread/testing.h; the split core uses gc_binding.h.
    # Enable both existing seams, so nursery retries are observed on both versions.
    for relative in ("src/thread/testing.h", "src/gc_binding.h"):
        header = runtime / relative
        if header.exists() and not header.read_text().startswith("#define SCOOP_THREAD_TESTING\n"):
            header.write_text("#define SCOOP_THREAD_TESTING\n" + header.read_text())
    report["runtime_files"] = {
        str(path.relative_to(runtime)): digest(path)
        for path in sorted(runtime.rglob("*"))
        if path.is_file()
    }
    source = repo / "tests/benchmarks/m44/progress"
    report["benchmark_sources"] = {
        path.name: path.read_text() for path in sorted(source.iterdir()) if path.is_file()
    }

    def run(command):
        command = list(map(str, command))
        started = time.perf_counter()
        result = subprocess.run(command, capture_output=True, text=True)
        report["commands"].append(
            {
                "command": command,
                "seconds": time.perf_counter() - started,
                "returncode": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
            }
        )
        (work / "build.json").write_text(json.dumps(report, indent=2) + "\n")
        result.check_returncode()

    for name in ("runner", "observe"):
        run(
            [
                "cc",
                "-std=c11",
                "-D_POSIX_C_SOURCE=200809L",
                "-O2",
                "-fno-lto",
                "-fno-common",
                "-fno-omit-frame-pointer",
                "-fno-optimize-sibling-calls",
                "-pthread",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-I",
                runtime / "include",
                "-I",
                runtime / "src",
                "-c",
                source / f"{name}.c",
                "-o",
                work / f"{name}.o",
            ]
        )
    run(["ar", "rcs", work / "libm44_progress.a", work / "runner.o", work / "observe.o"])
    command = [
        tools / "scoop",
        "build",
        source / "program.scoop",
        "--scoopc",
        tools / "scoopc",
        "--runtime-root",
        runtime,
        "--sysroot",
        work / "sysroot",
        "--cache-dir",
        args.cache.resolve(),
        "--target",
        args.target,
        "--target-dir",
        work / "target",
        "--profile",
        "release",
        "--library-path",
        work,
        "--emit",
        "all",
        "--dump-dir",
        work / "dump",
        "--message-format",
        "json",
        "-o",
        work / "progress",
    ]
    if args.unwind_prefix:
        command += ["--unwind-prefix", args.unwind_prefix.resolve()]
    run(command)
    report["binary"] = str(work / "progress")
    report["binary_sha256"] = digest(work / "progress")
    report["binary_bytes"] = (work / "progress").stat().st_size
    (work / "build.json").write_text(json.dumps(report, indent=2) + "\n")
    print(work / "build.json")


if __name__ == "__main__":
    main()
