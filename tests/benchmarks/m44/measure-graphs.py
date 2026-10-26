"""Alternate fixed graph binaries; preserve each process and collection sample."""

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


def measure(binary, graph, workers, nodes, rounds):
    environment = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("SCOOP_GC_", "SCOOP_BENCH_"))
    }
    environment.update(
        SCOOP_GC_STATS="1",
        SCOOP_GC_WORKERS=str(workers),
        SCOOP_BENCH_GRAPH=graph,
        SCOOP_BENCH_NODES=str(nodes),
        SCOOP_BENCH_ROUNDS=str(rounds),
    )
    cpu = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.perf_counter()
    result = subprocess.run(
        [str(binary)], capture_output=True, text=True, env=environment, check=True
    )
    elapsed = time.perf_counter() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    expected = (
        (nodes // 16) * (nodes // 16 - 1) // 2 * 16 * rounds
        if graph == "compact"
        else nodes * (nodes - 1) // 2 * (rounds if graph == "minor" else 1)
    )
    if result.stdout != f"{expected}\n":
        raise RuntimeError(f"{binary}: incorrect graph checksum: {result.stdout!r}")
    records = [json.loads(line) for line in result.stderr.splitlines()]
    phases = [item for item in records if item.get("scoop_mark_phase") == 1]
    stats = [item for item in records if item.get("scoop_gc") == 1]
    if len(phases) != rounds or len(stats) != 1 or len(records) != rounds + 1:
        raise RuntimeError(f"{binary}: missing or unexpected collection records")
    for index, phase in enumerate(phases):
        t0, t1, t2 = (phase[key] for key in ("stopping_ns", "collecting_ns", "running_ns"))
        if phase["round"] != index or not t0 <= t1 <= t2 or phase["moved_objects"] < 0:
            raise RuntimeError(f"{binary}: invalid collection observation: {phase}")
        if (phase["quiescence_ns"], phase["world_stopped_ns"], phase["request_to_running_ns"]) != (
            t1 - t0,
            t2 - t1,
            t2 - t0,
        ):
            raise RuntimeError(f"{binary}: inconsistent collection timing: {phase}")
        if graph == "compact" and not (
            phase["moved_objects"] > 0 and phase["source_regions"] > phase["target_regions"] > 0
        ):
            raise RuntimeError(f"{binary}: compact graph did not exercise selective evacuation")
        if graph == "minor" and not (
            phase["minor_collections"] == 1
            and phase["full_collections"] == phase["promotion_fallbacks"] == 0
            and 0 < phase["moved_objects"] < 2 * nodes
            and phase["traced_objects"] >= 2 * nodes
            and phase["promoted_bytes"] >= (2 * nodes - 1) * 24
            and phase["old_reference_slots"] >= nodes
            and phase["dirty_cards"] > 0
        ):
            raise RuntimeError(f"{binary}: minor graph missed promotion or remembered slots")
    gc = stats[0]
    if sum(gc["pause_buckets"]) < gc["minor_collections"] + gc["full_collections"]:
        raise RuntimeError(f"{binary}: inconsistent GC histogram")
    return {
        "seconds": elapsed,
        "cpu_seconds": after.ru_utime + after.ru_stime - cpu.ru_utime - cpu.ru_stime,
        "nodes_per_second": nodes / elapsed,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "collections": phases,
        "gc": gc,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", action="append", required=True, help="label=path")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=7)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--nodes", type=int, default=131071)
    parser.add_argument("--workers", type=int, default=4, choices=range(1, 9))
    parser.add_argument(
        "--graph", action="append", choices=("wide", "array", "chain", "compact", "minor")
    )
    args = parser.parse_args()
    if min(args.runs, args.rounds, args.nodes) < 1:
        parser.error("runs, rounds and nodes must be positive")
    if args.output.exists():
        parser.error("output already exists")
    binaries = {}
    for option in args.binary:
        label, separator, path = option.partition("=")
        if not separator or not label or label in binaries:
            parser.error("each binary needs a distinct label=path")
        binaries[label] = Path(path).resolve(strict=True)
    report = {
        "host": platform.platform(),
        "workers": args.workers,
        "nodes": args.nodes,
        "rounds": args.rounds,
        "runs": args.runs,
        "binaries": {},
        "samples": [],
        "summary": {},
        "notes": "Test-hook timing includes observation overhead; process throughput includes GC, graph construction and verification. Phase wall times overlap; worker CPU is separate. Small-sample p95 is descriptive only.",
    }
    for label, binary in binaries.items():
        report["binaries"][label] = {
            "path": str(binary),
            "bytes": binary.stat().st_size,
            "sha256": hashlib.file_digest(binary.open("rb"), "sha256").hexdigest(),
            "build": json.loads((binary.parent / "build.json").read_text()),
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    for graph in args.graph or ("wide", "array", "chain"):
        for iteration in range(-1, args.runs):
            order = list(binaries.items())
            if iteration % 2:
                order.reverse()
            for label, binary in order:
                sample = measure(binary, graph, args.workers, args.nodes, args.rounds)
                report["samples"].append(
                    {
                        "label": label,
                        "graph": graph,
                        "iteration": iteration,
                        "warmup": iteration == -1,
                        **sample,
                    }
                )
                args.output.write_text(json.dumps(report, indent=2) + "\n")
        for label in binaries:
            samples = [
                s
                for s in report["samples"]
                if s["label"] == label and s["graph"] == graph and not s["warmup"]
            ]
            phases = [phase for sample in samples for phase in sample["collections"]]
            metrics = {
                key: distribution([phase[key] for phase in phases])
                for key in phases[0]
                if isinstance(phases[0][key], int)
                and key
                not in (
                    "scoop_mark_phase",
                    "round",
                    "stopping_ns",
                    "collecting_ns",
                    "running_ns",
                )
            }
            metrics["process_seconds"] = distribution([s["seconds"] for s in samples])
            metrics["process_cpu_seconds"] = distribution([s["cpu_seconds"] for s in samples])
            metrics["nodes_per_second"] = distribution([s["nodes_per_second"] for s in samples])
            report["summary"][f"{label}/{graph}"] = metrics
            print(
                f"{label}/{graph}: update {metrics['update_ns']['median'] / 1e6:.3f} ms, request-to-running {metrics['request_to_running_ns']['median'] / 1e6:.3f} ms",
                flush=True,
            )
        args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
