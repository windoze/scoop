"""Check full-sized saved results against independent Python references."""

import argparse
import functools
import hashlib
import json
from pathlib import Path

import cases
from common import ROOT, write_json
from run import validate


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    records = json.loads((args.directory / "measurements.json").read_text())
    cases.mix = functools.cache(cases.mix)
    expected = {}
    for index, (name, n, _, _) in enumerate(cases.CASES):
        expected[name] = [cases.oracle(index, n, seed) for seed in range(42, 50)]
        print(f"full reference: {name}", flush=True)
    for run in records["runs"]:
        validate(run, expected[run["case"]])
    profile_path = args.directory / "profiles.json"
    profiles = json.loads(profile_path.read_text()) if profile_path.exists() else []
    for profile in profiles:
        validate(profile, expected[profile["case"]])
    repeat_path = args.directory / "repeat/measurements.json"
    repeat_runs = json.loads(repeat_path.read_text())["runs"] if repeat_path.exists() else []
    for run in repeat_runs:
        validate(run, expected[run["case"]])
    matched_sources = []
    for name, digest in records["source_sha256"].items():
        if Path(name).suffix not in {".scoop", ".go", ".java", ".c", ".toml"}:
            continue
        if hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != digest:
            raise ValueError(f"Measured input changed: {name}")
        matched_sources.append(name)
    write_json(
        args.directory / "full-verification.json",
        {
            "reference": "independent Python implementations/formulas at full measured sizes",
            "full_reference_checks": expected,
            "measurement_processes": len(records["runs"]),
            "profile_processes": len(profiles),
            "repeat_processes": len(repeat_runs),
            "measured_input_hashes_match": matched_sources,
            "result": "all passed",
        },
    )


if __name__ == "__main__":
    main()
