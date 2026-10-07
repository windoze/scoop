"""Measure a few Scoop programs through the ordinary CLI."""

import argparse
import json
import os
import platform
import statistics
import subprocess
import time
from pathlib import Path


def run(command, environment=None):
    start = time.perf_counter()
    result = subprocess.run(command, capture_output=True, text=True, env=environment)
    if result.returncode:
        raise RuntimeError(f"{command}: exit {result.returncode}\n{result.stderr}")
    return time.perf_counter() - start, result


def measure(binary, runs, gc_stats, compare_full):
    modes = ["nursery", "full_only"] if compare_full else ["default"]
    measurements = {mode: {"run_seconds": [], "gc": [], "stderr": []} for mode in modes}
    outputs = set()
    environment = dict(os.environ)
    for key in (
        "SCOOP_GC_STRESS_MOVE",
        "SCOOP_GC_STRESS_MINOR",
        "SCOOP_GC_FULL_ONLY",
        "SCOOP_GC_STATS",
    ):
        environment.pop(key, None)
    if gc_stats:
        environment["SCOOP_GC_STATS"] = "1"
    for _ in range(runs):
        for mode in modes:
            selected = dict(environment)
            if mode == "full_only":
                selected["SCOOP_GC_FULL_ONLY"] = "1"
            duration, result = run([str(binary)], selected)
            sample = measurements[mode]
            sample["run_seconds"].append(duration)
            sample["stderr"].append(result.stderr)
            outputs.add(result.stdout)
            if gc_stats:
                metrics = json.loads(result.stderr)
                if metrics.get("scoop_gc") != 1:
                    raise RuntimeError(f"{binary}: missing GC statistics")
                count = metrics["minor_collections"] + metrics["full_collections"]
                if sum(metrics["pause_buckets"]) != count:
                    raise RuntimeError(f"{binary}: inconsistent pause histogram")
                if mode == "full_only" and metrics["minor_collections"]:
                    raise RuntimeError(f"{binary}: full-only control performed minor collection")
                sample["gc"].append(metrics)
            elif result.stderr:
                raise RuntimeError(f"{binary}: unexpected stderr: {result.stderr}")
    if len(outputs) != 1:
        raise RuntimeError(f"{binary}: inconsistent output across runs or collectors")
    for sample in measurements.values():
        sample["median_seconds"] = statistics.median(sample["run_seconds"])
        sample["min_seconds"] = min(sample["run_seconds"])
        sample["max_seconds"] = max(sample["run_seconds"])
    primary = measurements[modes[0]]
    if compare_full:
        primary["full_only"] = measurements["full_only"]
    primary["stdout"] = outputs.pop()
    return primary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("tools", "sysroot", "runtime", "work"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--profile", choices=("debug", "release"), default="debug")
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--build-arg", action="append", default=[])
    parser.add_argument("--case", action="append")
    parser.add_argument("--gc-stats", action="store_true")
    parser.add_argument("--compare-full", action="store_true")
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("--runs must be positive")
    if args.compare_full and not args.gc_stats:
        parser.error("--compare-full requires --gc-stats")
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=False)
    tools = args.tools.resolve()
    report = {
        "revision": args.revision,
        "host": platform.platform(),
        "profile": args.profile,
        "runs": args.runs,
        "build_args": args.build_arg,
        "gc_stats": args.gc_stats,
        "compare_full": args.compare_full,
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
        case = {
            "name": source.stem,
            "cold_build_seconds": cold,
            "warm_build_seconds": warm,
            **measure(binary, args.runs, args.gc_stats, args.compare_full),
            "executable_bytes": binary.stat().st_size,
            "slib_bytes": Path(records[-1]["root"]["path"]).stat().st_size,
            "build_records": records,
        }
        report["cases"].append(case)
        (work / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"{source.stem}: {case['median_seconds']:.6f}s", flush=True)


if __name__ == "__main__":
    main()
