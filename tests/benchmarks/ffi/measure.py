"""Measure actual Scoop FFI and C loops with the same separately compiled callees."""

import argparse
import json
import os
import platform
import statistics
import subprocess
from pathlib import Path

MODES = (
    "c-scalar",
    "safe-direct",
    "leaf-direct",
    "c-pair",
    "safe-bridge",
    "leaf-bridge",
    "c-errno",
    "safe-errno",
    "leaf-errno",
)


def sample(binary, mode, threads, collect):
    environment = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("BENCH_", "SCOOP_GC_"))
    }
    environment.update(
        BENCH_MODE=str(mode),
        BENCH_THREADS=str(threads),
        BENCH_ITERATIONS="20000000" if collect else "2000000",
    )
    if collect:
        environment["BENCH_GC"] = "1"
    result = subprocess.run(
        [str(binary.resolve())],
        env=environment,
        capture_output=True,
        text=True,
        check=True,
        timeout=60,
    )
    if result.stderr:
        raise RuntimeError(result.stderr)
    value = json.loads(result.stdout)
    if collect and not value["collections"]:
        raise RuntimeError("GC measurement needs the instrumented runtime")
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--gc-binary", type=Path)
    parser.add_argument("--errno-only", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not args.errno_only and args.gc_binary is None:
        parser.error("--gc-binary is required unless --errno-only is selected")
    modes = (6, 7, 8) if args.errno_only else range(len(MODES))
    report = {"host": platform.platform(), "samples": [], "medians": {}}
    for threads in (1, 2, 4, 8):
        timings = {MODES[mode]: [] for mode in modes}
        for _ in range(3):
            for mode in modes:
                value = sample(args.binary, mode, threads, False)
                report["samples"].append(value)
                timings[MODES[mode]].append(value["ns_per_call"])
        report["medians"][str(threads)] = {
            name: statistics.median(values) for name, values in timings.items()
        }
        if not args.errno_only:
            for mode in (1, 2):
                report["samples"].append(sample(args.gc_binary, mode, threads, True))
        args.output.write_text(json.dumps(report, indent=2) + "\n")
        print(f"threads={threads}: {report['medians'][str(threads)]}", flush=True)


if __name__ == "__main__":
    main()
