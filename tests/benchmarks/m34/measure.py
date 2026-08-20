import argparse
import hashlib
import json
import os
import platform
import resource
import statistics
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("kind", choices=["standard", "interfaces", "poll"])
parser.add_argument("--runs", type=int, default=7)
args = parser.parse_args()
repo = Path.cwd()
host = "linux-gnu" if platform.system() == "Linux" else "darwin"
report = {
    "kind": args.kind,
    "host": platform.platform(),
    "runs": args.runs,
    "warmups_per_mode": 1,
    "order": "off/on on even rounds, on/off on odd rounds",
    "cases": [],
}
output = repo / f"tmp/m34/final-{args.kind}-{host}.json"
baseline = repo / f"tmp/m34/baseline-{host}-release{'-llvm22' if host == 'darwin' else ''}"
old_standard = json.loads((baseline / "report.json").read_text())
if args.kind == "standard":
    names = [
        "aggregate",
        "allocation",
        "arrays",
        "collections",
        "old-graph-large",
        "old-graph",
        "primitive",
        "survivors",
    ]
    expected = {case["name"]: case["stdout"] for case in old_standard["cases"]}
else:
    names = (
        ["known", "unknown", "converted-once", "converted-each"]
        if args.kind == "interfaces"
        else ["1-thread", "4-threads"]
    )
    value = 305419896
    if args.kind == "interfaces":
        for _ in range(2000000):
            first = (value ^ 17) ^ ((value << 13) & 0xFFFFFFFF)
            second = first ^ (first >> 17)
            value = (second ^ (second << 5)) & 0xFFFFFFFF
        expected = {name: str(value) + "\n" for name in names}


def binary(mode, name):
    if args.kind == "standard" and mode == "off":
        return baseline / name
    work = repo / f"tmp/m34/final-{args.kind}-{mode}-{host}"
    return work / (name if args.kind == "standard" else args.kind)


def sample(path, environment, name):
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.perf_counter()
    result = subprocess.run(
        [str(path)], env=environment, capture_output=True, text=True, timeout=120
    )
    elapsed = time.perf_counter() - started
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    if result.returncode:
        raise RuntimeError(f"{path}: {result.returncode}\n{result.stderr}")
    records = [json.loads(line) for line in result.stderr.splitlines()]
    gc = records[-1]
    assert gc["scoop_gc"] == 1
    assert sum(gc["pause_buckets"]) == gc["minor_collections"] + gc["full_collections"]
    row = {
        "wall_seconds": elapsed,
        "cpu_seconds": after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "gc": gc,
    }
    if args.kind == "interfaces":
        assert len(records) == 2
        row["kernel"] = records[0]
    else:
        assert len(records) == 1
    if args.kind == "poll":
        row["response"] = json.loads(result.stdout)
        assert row["response"]["collections"] > 0
        assert row["response"]["gc"] is True
    else:
        assert result.stdout == expected[name], (name, result.stdout, expected[name])
    return row


for index, name in enumerate(names):
    environment = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("SCOOP_GC_", "SCOOP_BENCH_", "BENCH_"))
    }
    environment["SCOOP_GC_STATS"] = "1"
    overrides = {"SCOOP_GC_STATS": "1"}
    if args.kind == "interfaces":
        overrides.update(SCOOP_BENCH_KIND=str(index), SCOOP_BENCH_COUNT="2000000")
    elif args.kind == "poll":
        overrides.update(
            BENCH_MODE="2",
            BENCH_THREADS="1" if index == 0 else "4",
            BENCH_GC="1",
            BENCH_ITERATIONS="100000000",
        )
    environment.update(overrides)
    item = {"name": name, "environment": overrides, "modes": {}}
    for mode in ["off", "on"]:
        path = binary(mode, name)
        item["modes"][mode] = {
            "binary": str(path),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "executable_bytes": path.stat().st_size,
            "warmup": sample(path, environment, name),
            "samples": [],
        }
    report["cases"].append(item)
    for run in range(args.runs):
        for mode in ["off", "on"] if run % 2 == 0 else ["on", "off"]:
            item["modes"][mode]["samples"].append(sample(binary(mode, name), environment, name))
        output.write_text(json.dumps(report, indent=2) + "\n")
    for data in item["modes"].values():
        times = [row["wall_seconds"] for row in data["samples"]]
        data["wall_median_seconds"] = statistics.median(times)
        data["wall_range_seconds"] = [min(times), max(times)]
    output.write_text(json.dumps(report, indent=2) + "\n")
    print(
        name,
        {
            mode: round(data["wall_median_seconds"] * 1000, 3)
            for mode, data in item["modes"].items()
        },
        flush=True,
    )
