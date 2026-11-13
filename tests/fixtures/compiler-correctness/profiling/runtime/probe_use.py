import contextlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

scoop, merger, source, directory, cache, sysroot, *native = sys.argv[1:]
work = Path(directory)
index = work / "profile.json"
data = json.loads(index.read_text())
maps = work / "saved maps"
maps.mkdir()
for ordinal, path in enumerate(data["maps"]):
    destination = maps / f"{ordinal}.json"
    shutil.copyfile(path, destination)
    data["maps"][ordinal] = f"saved maps/{ordinal}.json"
index.write_text(json.dumps(data))
output = work / "optimized"
dump = work / "feedback dump"
base = [
    scoop,
    "build",
    *native,
    source,
    "--sysroot",
    sysroot,
    "--cache-dir",
    cache,
    "--target-dir",
    str(work / "target"),
    "--library-path",
    str(work),
    "--message-format",
    "json",
]


def build(profile=index, *, required=True, expected=0, destination=output, mode="release"):
    command = [*base, "--profile", mode, "--profile-use", str(profile), "-o", str(destination)]
    if required:
        command.append("--profile-use-required")
    if expected == 0:
        command += ["--emit", "optimizations", "--dump-dir", str(dump)]
    result = subprocess.run(command, capture_output=True, timeout=120)
    assert result.returncode == expected, result
    assert result.stdout == b"", result
    records = []
    for line in result.stderr.splitlines():
        with contextlib.suppress(ValueError, UnicodeError):
            records.append(json.loads(line))
    if expected:
        return result
    return next(record for record in records if record.get("kind") == "executable")


def frequencies(result):
    report = json.loads((dump / result["root"]["identity"] / "optimizations.json").read_text())
    assert report["profile_use"]["consumers"], report
    return sorted(
        (
            event["heat"]["profile"]["frequency"]["calls"],
            event["heat"]["profile"]["frequency"]["entries"],
        )
        for event in report["events"]
        if event.get("heat", {}).get("source") == "profile"
    )


first = build()
summary = first["profile_use"]
assert any(cone["summary"]["consumers"] for cone in summary["source_cones"]), first
assert all(
    "/profiling/release/" in dep["path"].replace("\\", "/") for dep in first["dependencies"]
), first
before = frequencies(first)
assert before, first
reused = build()
assert reused["observations"]["child_invocations"] == [], reused
assert reused["profile_use"] == summary

# A feedback binary keeps the business result and contains no runtime probes.
empty = work / "no probes"
result = subprocess.run(
    [first["output"]],
    capture_output=True,
    env=dict(os.environ, SCOOP_PROFILE_DIR=str(empty)),
    timeout=30,
)
assert result.returncode == 0 and result.stdout == b"15\n" and result.stderr == b"", result
assert not empty.exists()

# New samples update counts without changing the selected index path.
raw = sorted((work / "profile 数据 % runs").glob("*/counts.profraw"))
subprocess.run(
    [merger, "merge", "-o", str(work / "counts.profdata"), *map(str, raw), *map(str, raw)],
    check=True,
    capture_output=True,
)
updated = build()
assert len(updated["observations"]["child_invocations"]) == 1, updated
assert frequencies(updated) == [(calls * 2, entries * 2) for calls, entries in before]

# A map is also a normal compiler input. The dataset owns this copy.
with (maps / "0.json").open("a") as target:
    target.write("\n")
assert len(build()["observations"]["child_invocations"]) == 1

# Required use is checked even when the fallback compilation is already cached.
unmatched = work / "unmatched.json"
unmatched.write_text(json.dumps(dict(data, maps=[])))
fallback = build(unmatched, required=False, destination=work / "fallback")
assert all(not cone["summary"]["consumers"] for cone in fallback["profile_use"]["source_cones"])
rejected = work / "rejected feedback"
rejected.write_bytes(b"previous executable")
failure = build(unmatched, expected=1, destination=rejected)
assert b"not consumed by any optimization" in failure.stderr, failure
assert rejected.read_bytes() == b"previous executable"

wrong = work / "wrong target.json"
wrong.write_text(json.dumps(dict(data, target="different-target")))
failure = build(wrong, expected=1, destination=rejected)
assert b"does not match" in failure.stderr, failure
assert rejected.read_bytes() == b"previous executable"

invalid = work / "invalid.json"
invalid.write_text(json.dumps(dict(data, counts="invalid.profdata")))
(work / "invalid.profdata").write_bytes(b"invalid indexed profile")
build(invalid, expected=1, destination=rejected)
assert rejected.read_bytes() == b"previous executable"
failure = build(expected=1, destination=rejected, mode="profiling")
assert b"--profile-use requires" in failure.stderr, failure
assert rejected.read_bytes() == b"previous executable"
sys.stdout.buffer.write(b"profiling feedback passed\n")
