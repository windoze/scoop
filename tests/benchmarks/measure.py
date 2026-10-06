"""Measure a few Scoop programs through the ordinary CLI."""

import argparse
import json
import platform
import statistics
import subprocess
import time
from pathlib import Path


def run(command):
    start = time.perf_counter()
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"{command}: exit {result.returncode}\n{result.stderr}")
    return time.perf_counter() - start, result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("tools", "sysroot", "runtime", "work"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--profile", choices=("debug", "release"), default="debug")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--build-arg", action="append", default=[])
    parser.add_argument("--case", action="append")
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("--runs must be positive")
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=False)
    tools = args.tools.resolve()
    report = {
        "revision": args.revision,
        "host": platform.platform(),
        "profile": args.profile,
        "runs": args.runs,
        "build_args": args.build_arg,
        "cases": [],
    }
    for source in sorted(Path(__file__).parent.glob("*.scoop")):
        if args.case and source.stem not in args.case:
            continue
        binary = work / source.stem
        command = [
            str(tools / "scoop"),
            "build",
            str(source.resolve()),
            "--scoopc",
            str(tools / "scoopc"),
            "--sysroot",
            str(args.sysroot.resolve()),
            "--runtime-root",
            str(args.runtime.resolve()),
            "--cache-dir",
            str(work / "cache"),
            "--profile",
            args.profile,
            "--message-format",
            "json",
            "-o",
            str(binary),
            *args.build_arg,
        ]
        cold, result = run(command)
        records = [json.loads(line) for line in result.stderr.splitlines()]
        warm, _ = run(command)
        samples, outputs, diagnostics = [], [], []
        for _ in range(args.runs):
            duration, result = run([str(binary)])
            samples.append(duration)
            outputs.append(result.stdout)
            diagnostics.append(result.stderr)
        if len(set(outputs)) != 1:
            raise RuntimeError(f"{source.name}: inconsistent output")
        case = {
            "name": source.stem,
            "cold_build_seconds": cold,
            "warm_build_seconds": warm,
            "run_seconds": samples,
            "median_seconds": statistics.median(samples),
            "min_seconds": min(samples),
            "max_seconds": max(samples),
            "executable_bytes": binary.stat().st_size,
            "slib_bytes": Path(records[-1]["root"]["path"]).stat().st_size,
            "stdout": outputs[0],
            "stderr": diagnostics,
            "build_records": records,
        }
        report["cases"].append(case)
        (work / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"{source.stem}: {case['median_seconds']:.6f}s", flush=True)


if __name__ == "__main__":
    main()
