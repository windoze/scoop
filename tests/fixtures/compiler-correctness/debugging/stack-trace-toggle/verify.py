"""Check the linked switch, actual snapshot slots and reuse of unchanged artifacts."""

import json
import subprocess
import sys
from pathlib import Path

scoop, scoopc, linker, sysroot, fixture, work, cache, *link_args = sys.argv[1:]
work = Path(work)
common = ["--sysroot", sysroot, "--cache-dir", cache, "--library-path", str(work), *link_args]


def checked(command):
    result = subprocess.run(command, capture_output=True, text=True, timeout=180)
    assert result.returncode == 0, (command, result.returncode, result.stdout, result.stderr)
    return result


def record(command):
    result = checked(command)
    assert not result.stdout, result.stdout
    records = [json.loads(line) for line in result.stderr.splitlines()]
    assert len(records) == 1, records
    return records[0]


def observe(path, enabled):
    result = checked([str(path), "enabled" if enabled else "disabled"])
    assert not result.stdout, result.stdout
    trace = result.stderr
    assert trace.count("x=Failure)") >= 3, trace
    assert "x=IndexOutOfBoundsException)" in trace, trace
    if enabled:
        assert trace.count("this object has not been thrown") == 1, trace
        assert trace.count("  at ") >= 6, trace
        assert "disabled" not in trace, trace
    else:
        assert trace.count("  <stack trace unavailable: disabled>\n") == 4, trace
        assert "  at " not in trace, trace
        assert "this object has not been thrown" not in trace, trace


def build(options):
    result = record(
        [
            scoop,
            "build",
            str(Path(fixture) / "program.scoop"),
            "--release",
            "--scoopc",
            scoopc,
            "--target-dir",
            str(work / "target"),
            "-o",
            str(work / "program"),
            "--message-format",
            "json",
            *common,
            *options,
        ]
    )
    assert result["kind"] == "executable", result
    return result


disabled = build([])
observe(disabled["output"], False)
artifact_path = Path(disabled["root"]["path"])
artifact_time = artifact_path.stat().st_mtime_ns
enabled = build(["--enable-stack-trace"])
observe(enabled["output"], True)
assert enabled["root"] == disabled["root"]
assert enabled["dependencies"] == disabled["dependencies"]
assert enabled["observations"]["child_invocations"] == [], enabled
assert enabled["link_plan_fingerprint"] != disabled["link_plan_fingerprint"]
assert artifact_path.stat().st_mtime_ns == artifact_time

# An explicit true equals the bare option; both reuse the completed executable.
output = Path(enabled["output"])
output_time = output.stat().st_mtime_ns
repeated = build(["--enable-stack-trace=true"])
assert repeated["link_plan_fingerprint"] == enabled["link_plan_fingerprint"]
assert output.stat().st_mtime_ns == output_time
assert repeated["observations"]["child_invocations"] == [], repeated

restored = build(["--enable-stack-trace=false"])
observe(restored["output"], False)
assert restored["link_plan_fingerprint"] == disabled["link_plan_fingerprint"]
assert restored["root"] == disabled["root"]
assert restored["observations"]["child_invocations"] == [], restored
assert artifact_path.stat().st_mtime_ns == artifact_time

# Both artifact consumers control the same already-compiled root and core library.
for direct in (False, True):
    for options, active in [
        ([], False),
        (["--enable-stack-trace"], True),
        (["--enable-stack-trace=false"], False),
    ]:
        output = work / f"linked-{direct}-{active}"
        command = [linker] if direct else [scoop, "link", "--message-format", "json"]
        command.extend(["--root-slib", str(artifact_path), "-o", str(output), *common, *options])
        for dependency in disabled["dependencies"]:
            command.extend(["--dep-slib" if direct else "--dependency-slib", dependency["path"]])
        checked(command)
        observe(output, active)
assert artifact_path.stat().st_mtime_ns == artifact_time
