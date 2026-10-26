"""Check physical callable removal and the remaining precise-root registrations."""

import json
import os
import subprocess
import sys
from pathlib import Path

report, ordinary, instrumented = map(Path, sys.argv[1:])
entries = json.loads(report.read_text())["symbols"]
removed = {"deadLeaf", "deadLeft", "deadRight"}
kept = {"Payload.number", "main"}
selected = [entry for entry in entries if entry["name"] in removed | kept]
assert {entry["name"] for entry in selected} == removed | kept

for binary in (ordinary, instrumented):
    data = binary.read_bytes()
    for entry in selected:
        present = entry["name"] in kept
        # The final symbol table and both registration kinds identify the body.
        assert (entry["symbol"].encode() + b"\0" in data) == present, entry
        assert (bytes.fromhex(entry["body"]) in data) == present, entry
    assert (b'{"scoop_startup":1' in data) == (binary == instrumented)

counts = None
for binary, stress in ((ordinary, None), (instrumented, "MOVE"), (instrumented, "MINOR")):
    environment = os.environ.copy()
    for name in ("SCOOP_GC_STRESS_MOVE", "SCOOP_GC_STRESS_MINOR", "SCOOP_GC_STATS"):
        environment.pop(name, None)
    if stress:
        environment["SCOOP_GC_STRESS_" + stress] = "1"
    output = subprocess.run(
        [binary], env=environment, capture_output=True, text=True, timeout=30, check=True
    )
    assert output.stdout == "init\n42\n", output
    if stress:
        record = json.loads(output.stderr)
        assert record["scoop_startup"] == 1, record
        current = record["counts"]
        assert current["callables"] > 0 and current["safepoints"] > 0, record
        assert current["safepoints"] == current["stackmap_records"], record
        assert counts is None or counts == current
        counts = current
    else:
        assert output.stderr == "", output
print("unreachable callables removed; dispatch and roots preserved")
