"""Verify work, then measure serially with interleaved independent process forks."""

import argparse
import hashlib
import json
import platform
import random
import statistics
from datetime import datetime
from pathlib import Path

from cases import CASES, oracle
from common import (
    JAVA_HOME,
    ROOT,
    SCOOP,
    commands,
    environment,
    execute,
    version,
    write_json,
)


def validate(result, expected=None):
    records = [json.loads(line) for line in result["stdout"].splitlines()]
    checks = [r["value"] for r in records if r["type"] == "check"]
    if len(checks) != 8 or (expected is not None and checks != expected):
        raise ValueError(f"incorrect results: {checks}, expected {expected}")
    samples = [r for r in records if r["type"] == "sample"]
    for sample in samples:
        calls = sample["calls"]
        checksum = 0
        for i in range(8):
            if (calls // 8 + (i < calls % 8)) & 1:
                checksum ^= checks[i]
        if sample["checksum"] != checksum:
            raise ValueError(f"incorrect measured checksum: {sample}")
        if calls == 0 and sample["index"] >= 0:
            raise ValueError("empty measurement")
    measured = [r for r in samples if r["index"] >= 0]
    return checks, samples, statistics.median(r["elapsed_ns"] / r["calls"] for r in measured)


def run_one(language, case, n, warmup, sample_ms, samples, label, command=None):
    env = environment()
    env.update(
        {
            "BENCH_CASE": str(case),
            "BENCH_N": str(n),
            "BENCH_SEED": "42",
            "BENCH_WARMUP_MS": str(warmup),
            "BENCH_SAMPLE_MS": str(sample_ms),
            "BENCH_SAMPLES": str(samples),
        }
    )
    result = execute(command or commands()[language], label, env)
    result["configuration"] = {key: value for key, value in env.items() if key.startswith("BENCH_")}
    return result


def verify(args):
    report = []
    for case, (name, _, small, _) in enumerate(CASES):
        if args.case and name not in args.case:
            continue
        expected = [oracle(case, small, seed) for seed in range(42, 50)]
        for language in args.language:
            result = run_one(language, case, small, 0, 1, 1, f"verify-{name}-{language}")
            checks, samples, _ = validate(result, expected)
            report.append(
                {
                    "case": name,
                    "language": language,
                    "n": small,
                    "checks": checks,
                    "samples": samples,
                }
            )
        print(f"verified {name}", flush=True)
    write_json(args.output / "verification.json", report)


def host():
    return {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "cpu": version(["sysctl", "-n", "machdep.cpu.brand_string"]),
        "memory_bytes": int(version(["sysctl", "-n", "hw.memsize"])),
        "logical_cpus": int(version(["sysctl", "-n", "hw.ncpu"])),
        "scoop": version([str(SCOOP), "--version"]),
        "go": version(["go", "version"]),
        "java": version([str(JAVA_HOME / "bin/java"), "-version"]),
        "java_flags": version([str(JAVA_HOME / "bin/java"), "-XX:+PrintFlagsFinal", "-version"]),
        "power": version(["pmset", "-g", "custom"]),
    }


def measure(args):
    output = args.output / "measurements.json"
    if output.exists():
        raise FileExistsError(f"Choose a fresh results directory: {output}")
    rng = random.Random(20261029)
    report = {
        "started": datetime.now().astimezone().isoformat(),
        "host": host(),
        "method": {
            "forks": args.forks,
            "samples_per_fork": args.samples,
            "sample_ms": args.sample_ms,
            "jit_warmup_ms": args.jit_warmup_ms,
            "aot_warmup_ms": 500,
            "gc": "defaults, Java JIT and Native Image -Xms64m -Xmx1g",
            "statistic": "median of independent process medians",
            "order_seed": 20261029,
        },
        "source_sha256": {},
        "runs": [],
        "summary": [],
    }
    for path in sorted(ROOT.rglob("*")):
        if (
            path.is_file()
            and path.suffix in {".scoop", ".go", ".java", ".c", ".py", ".toml"}
            and not set(path.relative_to(ROOT).parts) & {"build", "results"}
        ):
            report["source_sha256"][str(path.relative_to(ROOT))] = hashlib.sha256(
                path.read_bytes()
            ).hexdigest()
    for case, (name, n, _, description) in enumerate(CASES):
        if args.case and name not in args.case:
            continue
        expected = None
        for fork in range(args.forks):
            order = list(args.language)
            rng.shuffle(order)
            for language in order:
                warmup = args.jit_warmup_ms if language == "java-jit" else 500
                result = run_one(
                    language,
                    case,
                    n,
                    warmup,
                    args.sample_ms,
                    args.samples,
                    f"run-{name}-{language}-{fork}",
                )
                checks, samples, median = validate(result, expected)
                expected = checks
                result.update(
                    {
                        "case": name,
                        "language": language,
                        "fork": fork,
                        "n": n,
                        "checks": checks,
                        "samples": samples,
                        "median_ns": median,
                    }
                )
                report["runs"].append(result)
                write_json(output, report)
                print(
                    f"{name:20} {language:12} fork={fork + 1} {median / 1e6:10.3f} ms/call RSS={result['peak_rss_bytes'] / 2**20:8.1f} MiB",
                    flush=True,
                )
        summary = {"case": name, "description": description, "n": n, "languages": {}}
        for language in args.language:
            runs = [r for r in report["runs"] if r["case"] == name and r["language"] == language]
            medians = [r["median_ns"] for r in runs]
            summary["languages"][language] = {
                "median_ms": statistics.median(medians) / 1e6,
                "min_fork_ms": min(medians) / 1e6,
                "max_fork_ms": max(medians) / 1e6,
                "peak_rss_mib": statistics.median(r["peak_rss_bytes"] for r in runs) / 2**20,
                "cpu_seconds": statistics.median(r["cpu_seconds"] for r in runs),
            }
        report["summary"].append(summary)
        write_json(output, report)
    report["finished"] = datetime.now().astimezone().isoformat()
    write_json(output, report)


def startup(args):
    rng = random.Random(20261029)
    report = {
        "note": "warm filesystem cache; full suite no-work path; process launch/output/exit included",
        "runs": [],
        "summary": {},
    }
    for trial in range(33):
        order = list(args.language)
        rng.shuffle(order)
        for language in order:
            env = environment()
            env["BENCH_CASE"] = "-1"
            result = execute(commands()[language], f"startup-{language}-{trial}", env)
            if result["stdout"] != "ready\n":
                raise ValueError(result)
            if trial >= 3:
                report["runs"].append({"language": language, "trial": trial - 3, **result})
    for language in args.language:
        runs = [r for r in report["runs"] if r["language"] == language]
        report["summary"][language] = {
            "median_ms": statistics.median(r["wall_seconds"] for r in runs) * 1000,
            "min_ms": min(r["wall_seconds"] for r in runs) * 1000,
            "max_ms": max(r["wall_seconds"] for r in runs) * 1000,
            "peak_rss_mib": statistics.median(r["peak_rss_bytes"] for r in runs) / 2**20,
        }
    write_json(args.output / "startup.json", report)
    print(json.dumps(report["summary"], indent=2), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["verify", "measure", "startup"])
    parser.add_argument("--output", type=Path, default=ROOT / "results/2026-10-29")
    parser.add_argument("--language", action="append", choices=list(commands()))
    parser.add_argument("--case", action="append", choices=[c[0] for c in CASES])
    parser.add_argument("--forks", type=int, default=3)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--sample-ms", type=int, default=200)
    parser.add_argument("--jit-warmup-ms", type=int, default=5000)
    args = parser.parse_args()
    args.language = args.language or list(commands())
    if min(args.forks, args.samples, args.sample_ms) <= 0 or args.jit_warmup_ms < 0:
        parser.error("positive sample settings and nonnegative warmup required")
    {"verify": verify, "measure": measure, "startup": startup}[args.mode](args)


if __name__ == "__main__":
    main()
