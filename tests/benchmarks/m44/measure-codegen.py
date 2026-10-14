"""Alternate prebuilt allocation/write/poll binaries without concurrent build work."""

import argparse
import hashlib
import json
import math
import os
import platform
import resource
import statistics
import subprocess
import time
from pathlib import Path


def distribution(values):
    ordered = sorted(values)
    return {
        "count": len(values),
        "median": statistics.median(values),
        "p95": ordered[math.ceil(len(values) * 0.95) - 1],
        "min": ordered[0],
        "max": ordered[-1],
    }


def measure(case, environment):
    cpu = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.perf_counter()
    result = subprocess.run(
        [case["binary"]], capture_output=True, text=True, env=environment, check=True
    )
    elapsed = time.perf_counter() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if result.stdout != case["expected_stdout"]:
        raise RuntimeError(f"wrong checksum: {case['name']}: {result.stdout!r}")
    gc = json.loads(result.stderr)
    if gc.get("scoop_gc") != 1 or sum(gc["pause_buckets"]) != (
        gc["minor_collections"] + gc["full_collections"]
    ):
        raise RuntimeError("invalid GC statistics")
    return {
        "seconds": elapsed,
        "cpu_seconds": after.ru_utime + after.ru_stime - cpu.ru_utime - cpu.ru_stime,
        "iterations_per_second": case["iterations"] / elapsed,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "gc": gc,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=7)
    parser.add_argument("--workers", type=int, default=4, choices=range(1, 9))
    args = parser.parse_args()
    if args.runs < 7 or args.output.exists():
        parser.error("use at least seven runs and a new output path")
    build = json.loads(args.build.read_text())
    groups = {}
    for case in build["cases"]:
        with Path(case["binary"]).open("rb") as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != case["binary_sha256"]:
                raise RuntimeError("binary changed after build")
        groups.setdefault(case["name"], []).append(case)
    if not groups or any(len(cases) != 2 for cases in groups.values()):
        raise RuntimeError("the comparison requires two builds per workload")
    environment = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("SCOOP_GC_", "SCOOP_BENCH_"))
    }
    environment.update(SCOOP_GC_STATS="1", SCOOP_GC_WORKERS=str(args.workers))
    report = {
        "host": platform.platform(),
        "runs": args.runs,
        "workers": args.workers,
        "build": build,
        "samples": [],
        "summary": {},
    }
    for name, cases in groups.items():
        for iteration in range(-1, args.runs):
            for case in cases if iteration % 2 == 0 else reversed(cases):
                sample = {
                    "name": name,
                    "label": case["label"],
                    "iteration": iteration,
                    "warmup": iteration == -1,
                    **measure(case, environment),
                }
                report["samples"].append(sample)
            args.output.write_text(json.dumps(report, indent=2) + "\n")
        report["summary"][name] = {}
        for case in cases:
            samples = [
                sample
                for sample in report["samples"]
                if sample["name"] == name
                and sample["label"] == case["label"]
                and not sample["warmup"]
            ]
            report["summary"][name][case["label"]] = {
                key: distribution([sample[key] for sample in samples])
                for key in ("seconds", "cpu_seconds", "iterations_per_second")
            }
        args.output.write_text(json.dumps(report, indent=2) + "\n")
        print(name, report["summary"][name], flush=True)


if __name__ == "__main__":
    main()
