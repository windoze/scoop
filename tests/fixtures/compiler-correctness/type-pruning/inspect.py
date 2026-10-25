"""Check actual type removal and the remaining runtime registrations."""

import json
import os
import re
import subprocess
import sys
from pathlib import Path

dump, ordinary, instrumented = map(Path, sys.argv[1:])
lir = "\n".join(path.read_text() for path in dump.rglob("*") if path.is_file())
types = re.findall(r"^\s*td td\d+ (.+?) @(\S+) type-id=\d+", lir, re.MULTILINE)
discarded = ("DeadType", "DeadCycle", "DeadValue")
removed = [(name, symbol) for name, symbol in types if any(marker in name for marker in discarded)]
kept = [
    (name, symbol)
    for name, symbol in types
    if name.startswith(("n(c=dev.typepruning", "a(n(c=dev.typepruning"))
    and ("KeptType" in name or (name.startswith("a(") and "UsedValue" in name))
]
for marker in discarded:
    assert any(marker in name for name, _ in removed), (marker, types)
assert any("Holder" in name for name, _ in removed), removed
assert any("Holder" in name for name, _ in kept), kept
assert any("KeptType" in name for name, _ in kept), kept
for binary in (ordinary, instrumented):
    data = binary.read_bytes()
    for entries, present in ((removed, False), (kept, True)):
        for name, symbol in entries:
            assert (symbol.encode() + b"\0" in data) == present, (name, symbol)
    assert (b'{"scoop_startup":1' in data) == (binary == instrumented)

for binary, stress, arguments in (
    (ordinary, None, []),
    (instrumented, "MOVE", []),
    (instrumented, "MINOR", ["other"]),
):
    environment = os.environ.copy()
    for name in ("SCOOP_GC_STRESS_MOVE", "SCOOP_GC_STRESS_MINOR", "SCOOP_GC_STATS"):
        environment.pop(name, None)
    if stress:
        environment["SCOOP_GC_STRESS_" + stress] = "1"
    output = subprocess.run(
        [binary, *arguments],
        env=environment,
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    assert output.stdout == "init\n42\n" + ("false\n" if arguments else "true\n"), output
    if stress:
        record = json.loads(output.stderr)
        assert record["counts"]["types"] > 0, record
        assert record["counts"]["safepoints"] == record["counts"]["stackmap_records"]
    else:
        assert output.stderr == "", output
print("unreachable types removed; live generic, boxed and interface types preserved")
