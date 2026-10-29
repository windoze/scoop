"""Collect Scoop GC statistics separately from the ordinary timing runs."""

import argparse
import json
from pathlib import Path

from cases import CASES
from common import ROOT, commands, environment, execute, write_json


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, default=ROOT / "results/2026-10-29", nargs="?")
    args = parser.parse_args()
    timing = json.loads((args.directory / "measurements.json").read_text())
    records = []
    for case, (name, n, _, _) in enumerate(CASES):
        env = environment()
        env.update(
            {
                "BENCH_CASE": str(case),
                "BENCH_N": str(n),
                "BENCH_SEED": "42",
                "BENCH_WARMUP_MS": "0",
                "BENCH_SAMPLE_MS": "0",
                "BENCH_SAMPLES": "0",
                "SCOOP_GC_STATS": "1",
            }
        )
        result = execute(commands()["scoop"], f"gc-{name}", env)
        checks = [
            json.loads(line)["value"]
            for line in result["stdout"].splitlines()
            if json.loads(line)["type"] == "check"
        ]
        expected = next(r["checks"] for r in timing["runs"] if r["case"] == name)
        if checks != expected:
            raise ValueError(name)
        stats = [
            json.loads(line)
            for line in result["stderr"].splitlines()
            if line.startswith('{"scoop_gc":')
        ]
        if len(stats) != 1:
            raise ValueError(f"Expected GC record for {name}")
        records.append({"case": name, "kernel_calls": 8, "stats": stats[0], **result})
        write_json(args.directory / "scoop-gc.json", records)
        print(f"{name}: {stats[0].get('allocated_bytes')} bytes / 8 calls", flush=True)


if __name__ == "__main__":
    main()
