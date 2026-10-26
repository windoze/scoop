"""Alternate fixed binaries and validate actual post-GC worker progress samples."""

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


def validate(value, mode, threads, iterations):
    if (
        value.get("scoop_progress") != 1
        or (value["mode"], value["threads"], value["iterations"]) != (mode, threads, iterations)
        or value["probe_every"] != 1024
        or len(value["workers"]) != threads
        or not value["collections"]
    ):
        raise RuntimeError("missing or inconsistent benchmark configuration")
    for worker in value["workers"]:
        if (
            worker["checksum"] != iterations * (iterations + 1) // 2
            or worker["probes"] != 1 + (iterations + 1023) // 1024
        ):
            raise RuntimeError("incorrect worker result or missing progress probes")
    if mode == 1 and value["allocated_bytes"] < threads * iterations * 24:
        raise RuntimeError("allocation workload did not allocate its Item objects")
    if mode == 1 and value["nursery_retries"] == 0:
        raise RuntimeError("allocation observation needs the enabled nursery retry hook")
    observations = {
        key: []
        for key in (
            "quiescence_ns",
            "world_stopped_ns",
            "request_to_running_ns",
            "progress_gap_ns",
            "running_to_progress_ns",
        )
    }
    previous_running = 0
    for collection in value["collections"]:
        t0, t1, t2 = (collection[k] for k in ("stopping_ns", "collecting_ns", "running_ns"))
        if not previous_running <= t0 <= t1 <= t2 or len(collection["progress"]) != threads:
            raise RuntimeError("invalid collection boundary order or worker count")
        previous_running = t2
        observations["quiescence_ns"].append(t1 - t0)
        observations["world_stopped_ns"].append(t2 - t1)
        observations["request_to_running_ns"].append(t2 - t0)
        for last, before, first, after in collection["progress"]:
            if last == 0:
                if (before, first, after) != (0, 0, 0):
                    raise RuntimeError("inactive worker has partial progress data")
                continue
            if not (last <= t1 <= t2 <= first and 0 <= before < after <= iterations):
                raise RuntimeError("worker did not make observed progress after collection")
            observations["progress_gap_ns"].append(first - last)
            observations["running_to_progress_ns"].append(first - t2)
    if not observations["progress_gap_ns"]:
        raise RuntimeError("no collection observed an active worker")
    return observations


def sample(binary, mode, threads, iterations, workers):
    environment = {
        k: v
        for k, v in os.environ.items()
        if not k.startswith(("BENCH_", "SCOOP_GC_", "SCOOP_BENCH_"))
    }
    environment.update(
        BENCH_MODE=str(mode),
        BENCH_THREADS=str(threads),
        BENCH_ITERATIONS=str(iterations),
        SCOOP_GC_STATS="1",
        SCOOP_GC_WORKERS=str(workers),
    )
    cpu = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.perf_counter()
    result = subprocess.run(
        [str(binary)], env=environment, capture_output=True, text=True, timeout=120, check=True
    )
    seconds = time.perf_counter() - started
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    value = json.loads(result.stdout)
    observations = validate(value, mode, threads, iterations)
    gc = json.loads(result.stderr)
    if (
        gc.get("scoop_gc") != 1
        or sum(gc["pause_buckets"]) < gc["minor_collections"] + gc["full_collections"]
    ):
        raise RuntimeError("invalid runtime GC statistics")
    return {
        "seconds": seconds,
        "cpu_seconds": after.ru_utime + after.ru_stime - cpu.ru_utime - cpu.ru_stime,
        "iterations_per_second": iterations * threads / (value["elapsed_ns"] / 1e9),
        "observation": value,
        "runtime_gc": gc,
        "distributions": {key: distribution(values) for key, values in observations.items()},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", action="append", required=True, help="label=path")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=7)
    parser.add_argument("--threads", type=int, choices=(1, 4), action="append")
    parser.add_argument("--mode", type=int, choices=(0, 1), action="append")
    parser.add_argument("--workers", type=int, choices=range(1, 9), default=4)
    args = parser.parse_args()
    if args.runs < 7 or args.output.exists():
        parser.error("use at least seven runs and a new output path")
    binaries = {}
    report = {
        "host": platform.platform(),
        "runs": args.runs,
        "workers": args.workers,
        "binaries": {},
        "samples": [],
        "summary": {},
    }
    for option in args.binary:
        label, separator, path = option.partition("=")
        if not separator or not label or label in binaries:
            parser.error("each binary needs a distinct label=path")
        binary = Path(path).resolve(strict=True)
        build = json.loads((binary.parent / "build.json").read_text())
        with binary.open("rb") as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != build["binary_sha256"]:
                raise RuntimeError("binary changed after its recorded build")
        binaries[label] = binary
        report["binaries"][label] = build
    for mode in args.mode or (0, 1):
        iterations = 100000000 if mode == 0 else 2000000
        for threads in args.threads or (1, 4):
            for iteration in range(-1, args.runs):
                order = list(binaries.items())
                if iteration % 2:
                    order.reverse()
                for label, binary in order:
                    result = sample(binary, mode, threads, iterations, args.workers)
                    report["samples"].append(
                        {
                            "label": label,
                            "mode": mode,
                            "threads": threads,
                            "iteration": iteration,
                            "warmup": iteration == -1,
                            **result,
                        }
                    )
                args.output.write_text(json.dumps(report, indent=2) + "\n")
            for label in binaries:
                samples = [
                    s
                    for s in report["samples"]
                    if s["label"] == label
                    and s["mode"] == mode
                    and s["threads"] == threads
                    and not s["warmup"]
                ]
                observations = [
                    validate(s["observation"], mode, threads, iterations) for s in samples
                ]
                summary = {
                    key: distribution(
                        [value for observation in observations for value in observation[key]]
                    )
                    for key in observations[0]
                }
                summary.update(
                    {
                        key: distribution([s[key] for s in samples])
                        for key in ("seconds", "cpu_seconds", "iterations_per_second")
                    }
                )
                summary["nursery_retries"] = distribution(
                    [s["observation"]["nursery_retries"] for s in samples]
                )
                report["summary"][f"{label}/{mode}/{threads}"] = summary
                print(
                    label,
                    mode,
                    threads,
                    "progress gap median ns",
                    summary["progress_gap_ns"]["median"],
                    flush=True,
                )
            args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
