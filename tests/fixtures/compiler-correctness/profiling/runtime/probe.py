import json
import os
import subprocess
import sys
from pathlib import Path

compiler, merger, target, directory, instrumented, ordinary, *maps = sys.argv[1:]
work = Path(directory)


def run(binary, output, args=(), cwd=None, expected=0):
    environment = dict(os.environ, SCOOP_PROFILE_DIR=str(output))
    environment.pop("SCOOP_GC_STATS", None)
    result = subprocess.run(
        [binary, *args], env=environment, cwd=cwd, capture_output=True, timeout=30
    )
    assert result.returncode == expected, result
    assert result.stdout == b"15\n", result
    return result


runs = work / "profile 数据 % runs"
assert run(instrumented, runs).stderr == b""
assert run(instrumented, runs, ["exit"], expected=7).stderr == b""
raw = sorted(runs.glob("*/counts.profraw"))
assert len(raw) == 2 and all(path.stat().st_size > 0 for path in raw), raw
snapshots = {path: path.read_bytes() for path in raw}
finishes = []
for path in sorted(runs.glob("*/runtime.jsonl")):
    records = [json.loads(line) for line in path.read_text().splitlines()]
    startup = [record for record in records if record.get("scoop_startup") == 1]
    gc = [record for record in records if record.get("scoop_gc") == 1]
    finish = [record for record in records if record.get("event") == "finish"]
    assert len(startup) == len(gc) == len(finish) == 1, records
    assert startup[0]["total_ns"] == sum(startup[0]["phases_ns"].values()) > 0
    assert gc[0]["allocated_bytes"] > 0
    assert finish[0]["runtime_elapsed_ns"] > 0
    finishes.append((finish[0]["reason"], finish[0]["exit_code"]))
assert sorted(finishes) == [("exit", 7), ("return", 0)], finishes

counts = work / "counts.profdata"
subprocess.run([merger, "merge", "-o", str(counts), *map(str, raw)], check=True)
index = work / "profile.json"
index.write_text(json.dumps({"schema": 1, "target": target, "counts": counts.name, "maps": maps}))
result = subprocess.run(
    [compiler, "profile-summary", str(index), "--message-format", "json"],
    capture_output=True,
    check=True,
    timeout=30,
)
assert result.stderr == b"", result.stderr
summary = json.loads(result.stdout)
assert summary["kind"] == "profile-summary" and summary["target"] == target
bodies = {body["name"]: body for body in summary["bodies"]}
for name, count in {"main": 2, "choose": 6, "leaf": 8004, "worker": 8, "notObserved": 0}.items():
    assert bodies[name]["positions"][0]["count"] == count, bodies[name]
assert any(
    position.get("frequency") == {"missing": "zero-entry"}
    for position in bodies["notObserved"]["positions"]
), bodies["notObserved"]
assert any(
    position.get("frequency", {}).get("calls") == 6 and position["frequency"]["entries"] == 2
    for position in bodies["main"]["positions"]
), bodies["main"]
core_maps = [Path(path) for path in maps if path.endswith("scoop.core.slib.profile.json")]
assert core_maps, maps
core_ids = {
    body["key"]["body"] for path in core_maps for body in json.loads(path.read_text())["bodies"]
}
assert any(
    body["key"]["body"] in core_ids and (body["positions"][0].get("count") or 0) > 0
    for body in summary["bodies"]
)

# The default is relative to the launched program's cwd, never its build cwd.
other = work / "other cwd"
other.mkdir()
environment = dict(os.environ)
environment.pop("SCOOP_PROFILE_DIR", None)
environment.pop("SCOOP_GC_STATS", None)
result = subprocess.run([instrumented], cwd=other, env=environment, capture_output=True, timeout=30)
assert result.returncode == 0 and result.stdout == b"15\n" and result.stderr == b"", result
assert len(list((other / "scoop-profile").glob("*/counts.profraw"))) == 1
assert all(path.read_bytes() == before for path, before in snapshots.items())

# A failed profile output reports the I/O failure and preserves program behavior.
blocked = work / "not-a-directory"
blocked.write_text("output path is a file\n")
failed_output = run(instrumented, blocked / "child")
assert b"scoop profiling: cannot create output" in failed_output.stderr, failed_output

release_output = work / "release-profile"
assert run(ordinary, release_output).stderr == b""
assert not release_output.exists()
content = Path(ordinary).read_bytes()
for marker in (b"scoop profiling:", b'{"scoop_profile":1', b"__llvm_prf_cnts", b".lprfc"):
    assert marker not in content, marker
sys.stdout.buffer.write(b"profiling runtime passed\n")
