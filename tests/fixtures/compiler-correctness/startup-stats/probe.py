import json
import os
import subprocess
import sys
from pathlib import Path

PHASES = {
    "loaded_images",
    "collect_records",
    "code_and_types",
    "storage_and_units",
    "stackmaps",
    "runtime_init",
    "eager_init",
}
COUNTS = {
    "images",
    "loaded_images",
    "types",
    "callables",
    "safepoints",
    "stackmap_records",
    "scan_programs",
    "static_storages",
    "static_roots",
    "immortals",
    "initialization_units",
    "eager_units",
    "context_keys",
}


def check_binary(binary, enabled):
    content = Path(binary).read_bytes()
    for marker in (
        b'{"scoop_startup":1',
        b"cannot measure startup time",
        b"scoop_image_scan_program_count",
    ):
        assert (marker in content) == enabled, (binary, marker, enabled)


def run(binary, enabled, *, nonzero=False, gc=False):
    environment = os.environ.copy()
    # An environment variable cannot enable code that was not compiled in.
    environment["SCOOP_STARTUP_STATS"] = "1"
    environment.pop("SCOOP_GC_STATS", None)
    if gc:
        environment["SCOOP_GC_STATS"] = "1"
    arguments = [binary, "nonzero"] if nonzero else [binary]
    result = subprocess.run(arguments, env=environment, capture_output=True, text=True, timeout=30)
    assert result.returncode == (7 if nonzero else 0), result
    assert result.stdout == "7\n", result.stdout
    lines = result.stderr.splitlines()
    assert lines.pop(0) == "startup eager", result.stderr
    record = None
    if enabled:
        record = json.loads(lines.pop(0))
        assert set(record) == {"scoop_startup", "total_ns", "phases_ns", "counts"}
        assert record["scoop_startup"] == 1
        phases = record["phases_ns"]
        counts = record["counts"]
        assert set(phases) == PHASES and set(counts) == COUNTS, record
        for value in [record["total_ns"], *phases.values(), *counts.values()]:
            assert type(value) is int and value >= 0, record
        assert sum(phases.values()) == record["total_ns"] > 0, record
        assert counts["images"] >= 2 and counts["loaded_images"] >= 1, counts
        assert counts["types"] > 0 and counts["callables"] > 0, counts
        assert counts["safepoints"] == counts["stackmap_records"] > 0, counts
        assert counts["static_roots"] <= counts["static_storages"], counts
        assert 1 <= counts["eager_units"] <= counts["initialization_units"], counts
    assert lines.pop(0) == "startup main", result.stderr
    if gc:
        assert json.loads(lines.pop(0))["scoop_gc"] == 1, result.stderr
    assert not lines, result.stderr
    return record


ordinary, ordinary_again, stats, combined, linked, low_linked = sys.argv[1:]
for binary in [ordinary, ordinary_again]:
    check_binary(binary, False)
    run(binary, False)
for binary in [stats, combined, linked, low_linked]:
    check_binary(binary, True)
    run(binary, True)
baseline = run(stats, True, nonzero=True)
assert run(stats, True, gc=True)["counts"] == baseline["counts"]
print("startup metrics passed")
