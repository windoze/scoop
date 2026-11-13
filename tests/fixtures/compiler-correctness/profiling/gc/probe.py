import json
import os
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

binary, directory = sys.argv[1:]
work = Path(directory)


def run(name, arguments, expected):
    destination = work / name
    environment = {
        key: value for key, value in os.environ.items() if not key.startswith("SCOOP_GC_")
    }
    environment.update(
        SCOOP_PROFILE_DIR=str(destination),
        SCOOP_GC_INCREMENTAL="1",
        SCOOP_GC_WORKERS="1",
        SCOOP_GC_NURSERY_MAX_MIB="64",
    )
    result = subprocess.run([binary, *arguments], env=environment, capture_output=True, timeout=60)
    assert result.returncode == 0 and result.stdout == expected and not result.stderr, result
    paths = list(destination.glob("*/runtime.jsonl"))
    assert len(paths) == 1, paths
    records = [json.loads(line) for line in paths[0].read_text().splitlines()]
    return records


records = run("timeline", [], b"262144\n")
trace = next(record for record in records if record.get("event") == "gc-trace")
events = [
    record for record in records if record.get("event", "").startswith("gc-") and record != trace
]
assert trace["events"] == len(events) and trace["dropped"] == trace["clock_failures"] == 0, trace
slices = {event["slice"]: event for event in events if event["event"] == "gc-slice"}
begins = {event["cycle"]: event for event in events if event["event"] == "gc-cycle-begin"}
ends = {event["cycle"]: event for event in events if event["event"] == "gc-cycle-end"}
phases = defaultdict(list)
cycle_slices = defaultdict(set)
for event in events:
    pause = slices[event["slice"]]
    if event["event"] == "gc-slice":
        assert pause["requested_ns"] <= pause["stopped_ns"] <= pause["resumed_ns"], pause
        assert pause["stop_wait_ns"] == pause["stopped_ns"] - pause["requested_ns"], pause
        assert pause["pause_ns"] == pause["resumed_ns"] - pause["stopped_ns"], pause
        continue
    cycle_slices[event["cycle"]].add(event["slice"])
    if event["event"] == "gc-phase":
        assert pause["stopped_ns"] <= event["begin_ns"] <= event["end_ns"] <= pause["resumed_ns"], (
            event
        )
        phases[event["cycle"], event["slice"]].append(event)
    else:
        assert pause["stopped_ns"] <= event["at_ns"] <= pause["resumed_ns"], event
assert begins.keys() == ends.keys(), (begins, ends)
assert any(
    begin["incremental"] and len(cycle_slices[cycle]) >= 3 and ends[cycle]["status"] == "restarted"
    for cycle, begin in begins.items()
), (begins, ends, cycle_slices)
assert any(end["status"] == "completed" for end in ends.values())
assert all(begin["at_ns"] <= ends[cycle]["at_ns"] for cycle, begin in begins.items())
for group in phases.values():
    by_name = {event["phase"]: event for event in group}
    for outer, inner in [("plan", "copy"), ("reclaim", "vm-return")]:
        if inner in by_name:
            parent, child = by_name[outer], by_name[inner]
            assert parent["begin_ns"] <= child["begin_ns"] <= child["end_ns"] <= parent["end_ns"]
assert {event["phase"] for group in phases.values() for event in group} == {
    "roots",
    "remembered",
    "mark",
    "plan",
    "copy",
    "update",
    "reclaim",
    "vm-return",
}
totals = next(record for record in records if record.get("scoop_gc") == 1)
for counter in ["root_slots", "mark_tasks", "traced_objects", "copied_bytes"]:
    assert sum(pause[counter] for pause in slices.values()) == totals[counter], (counter, totals)
assert sum(pause["worker_cpu_ns"] for pause in slices.values()) == sum(totals["worker_cpu_ns"])

limited = run("limited", ["fill"], b"1100\n")
trace = next(record for record in limited if record.get("event") == "gc-trace")
assert (
    trace["events"] == trace["capacity"] and trace["dropped"] > 0 and trace["clock_failures"] == 0
), trace
totals = next(record for record in limited if record.get("scoop_gc") == 1)
assert totals["full_collections"] >= 1100, totals
assert limited[-1]["event"] == "finish" and limited[-1]["exit_code"] == 0
sys.stdout.buffer.write(b"profiling GC timeline passed\n")
