"""Compare two prebuilt programs with alternating, independently measured runs."""

import argparse
import json
import os
import platform
import statistics
import sys
from datetime import UTC, datetime
from pathlib import Path

from process_metrics import gc_statistics, run_process

GC_CONTROLS = (
    "SCOOP_GC_STRESS_MOVE",
    "SCOOP_GC_STRESS_MINOR",
    "SCOOP_GC_FULL_ONLY",
    "SCOOP_GC_STATS",
)
METRICS = (
    "wall_seconds",
    "cpu_seconds",
    "user_cpu_seconds",
    "system_cpu_seconds",
    "peak_rss_bytes",
)


def arguments(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for variant in ("baseline", "candidate"):
        parser.add_argument(f"--{variant}", type=Path, required=True)
        parser.add_argument(f"--{variant}-label", default=variant)
        parser.add_argument(f"--{variant}-env", action="append", default=[])
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=7)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--arg", action="append", default=[])
    parser.add_argument("--expected-stdout", type=Path)
    parser.add_argument("--gc-stats", action="store_true")
    args = parser.parse_args(argv)
    if args.runs < 1 or args.warmups < 0:
        parser.error("--runs must be positive and --warmups must be nonnegative")
    for variant in ("baseline", "candidate"):
        overrides = {}
        for assignment in getattr(args, f"{variant}_env"):
            key, separator, value = assignment.partition("=")
            if not separator or not key or "\0" in assignment:
                parser.error(f"--{variant}-env requires NAME=VALUE")
            overrides[key] = value
        setattr(args, f"{variant}_env", overrides)
    return args


def variants_for(args):
    variants = {}
    for name in ("baseline", "candidate"):
        binary = getattr(args, name).resolve()
        overrides = getattr(args, f"{name}_env")
        environment = dict(os.environ)
        for key in GC_CONTROLS:
            environment.pop(key, None)
        environment.update(overrides)
        if args.gc_stats:
            environment["SCOOP_GC_STATS"] = "1"
        recorded = {key: environment.get(key) for key in GC_CONTROLS}
        recorded.update({key: environment[key] for key in overrides})
        variants[name] = {
            "command": [str(binary), *args.arg],
            "label": getattr(args, f"{name}_label"),
            "executable_bytes": binary.stat().st_size,
            "environment": environment,
            "environment_overrides": recorded,
        }
    return variants


def summarize(samples):
    grouped = {
        name: [sample for sample in samples if sample["variant"] == name]
        for name in ("baseline", "candidate")
    }
    summary = {}
    for metric in METRICS:
        baseline = statistics.median(sample[metric] for sample in grouped["baseline"])
        candidate = statistics.median(sample[metric] for sample in grouped["candidate"])
        summary[metric] = {
            "baseline_median": baseline,
            "candidate_median": candidate,
            "baseline_over_candidate": baseline / candidate if candidate else None,
        }
    summary["paired_wall_ratios"] = [
        baseline["wall_seconds"] / candidate["wall_seconds"]
        for baseline, candidate in zip(grouped["baseline"], grouped["candidate"], strict=True)
    ]
    return summary


def compare(args):
    variants = variants_for(args)
    expected = args.expected_stdout.read_bytes() if args.expected_stdout else None
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=False)
    report = {
        "schema": 1,
        "started_at_utc": datetime.now(UTC).isoformat(),
        "host": platform.platform(),
        "machine": platform.machine(),
        "logical_cpus": os.cpu_count(),
        "runs": args.runs,
        "warmups": args.warmups,
        "gc_stats": args.gc_stats,
        "expected_stdout": str(args.expected_stdout.resolve()) if args.expected_stdout else None,
        "variants": {
            name: {key: value for key, value in variant.items() if key != "environment"}
            for name, variant in variants.items()
        },
        "warmup_samples": [],
        "samples": [],
    }
    for phase, count, samples in (
        ("warmup", args.warmups, report["warmup_samples"]),
        ("measured", args.runs, report["samples"]),
    ):
        for pair in range(count):
            order = ("baseline", "candidate") if pair % 2 == 0 else ("candidate", "baseline")
            for name in order:
                variant = variants[name]
                stem = f"{phase}-{pair:03d}-{name}"
                stdout = work / f"{stem}.stdout"
                stderr = work / f"{stem}.stderr"
                measured = run_process(variant["command"], variant["environment"], stdout, stderr)
                output = stdout.read_bytes()
                if expected is None:
                    expected = output
                elif output != expected:
                    raise RuntimeError(f"{name}: stdout differs from expected output; see {stdout}")
                sample = {
                    "variant": name,
                    "pair": pair,
                    "stdout": stdout.name,
                    "stderr": stderr.name,
                    **measured,
                }
                if args.gc_stats:
                    sample["gc"] = gc_statistics(stderr.read_text(errors="replace"), stem)
                    if (
                        variant["environment"].get("SCOOP_GC_FULL_ONLY") == "1"
                        and sample["gc"]["minor_collections"]
                    ):
                        raise RuntimeError(f"{name}: full-only control performed minor collection")
                samples.append(sample)
    report["summary"] = summarize(report["samples"])
    (work / "report.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    return report


def main():
    args = arguments()
    try:
        report = compare(args)
    except (OSError, RuntimeError) as error:
        print(error, file=sys.stderr)
        return 1
    wall = report["summary"]["wall_seconds"]
    print(
        f"baseline {wall['baseline_median']:.6f}s; candidate {wall['candidate_median']:.6f}s; "
        f"ratio {wall['baseline_over_candidate']:.4f}; report {args.work.resolve() / 'report.json'}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
