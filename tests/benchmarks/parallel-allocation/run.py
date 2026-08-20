"""Measure existing one-mutator and four-mutator allocation executables."""

import argparse
import json
import platform
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from measure import measure  # noqa: E402


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--single", type=Path, required=True)
    parser.add_argument("--parallel", type=Path, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=7)
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("--runs must be positive")
    report = {"host": platform.platform(), "revision": args.revision, "samples": []}
    for name, binary, expected in (
        ("single", args.single, "1599960000\n"),
        ("parallel", args.parallel, "2000000\n"),
    ):
        for statistics in (False, True):
            result = measure(binary.resolve(), args.runs, statistics, False)
            if result["stdout"] != expected:
                raise RuntimeError(f"{name}: unexpected result {result['stdout']!r}")
            report["samples"].append({"case": name, "statistics": statistics, **result})
            args.output.write_text(json.dumps(report, indent=2) + "\n")
            print(f"{name}, statistics={statistics}: {result['median_seconds']:.6f}s", flush=True)


if __name__ == "__main__":
    main()
