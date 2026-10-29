"""Observe immutable first-throw traces and process termination through real executables."""

import re
import subprocess
import sys


def run(path, expected):
    result = subprocess.run([path], capture_output=True, timeout=30)
    assert result.returncode == expected, (result.returncode, result.stderr)
    assert result.stdout == b"", result.stdout
    return result.stderr.decode()


history = run(sys.argv[1], 0)
reports = re.findall(r"(?m)^\S[^\n]*\n(?:  [^\n]*\n)*", history)
assert "".join(reports) == history, history
assert len(reports) == 4, history
assert "stack not captured: this object has not been thrown" in reports[0], history
assert "x=Failure)" in reports[1].splitlines()[0], history
assert reports[1] == reports[2] == reports[3], history
assert reports[1].count("  at ") >= 2, history
assert "caller boundary unavailable" not in history, history
assert "unwind stopped" not in history, history

uncaught = run(sys.argv[2], 1)
assert uncaught.startswith("scoop: uncaught exception: "), uncaught
assert "x=IllegalStateException)" in uncaught.splitlines()[0], uncaught
assert uncaught.count("  at ") >= 2, uncaught

trap = run(sys.argv[3], 1)
assert trap.startswith("scoop: trap: array size overflow\n"), trap
assert trap.count("  at ") >= 2, trap

for trace in [reports[1], uncaught, trap]:
    assert re.search(r"\b(?:uuid|build-id)=[0-9a-f]+, link=0x[0-9a-f]+", trace), trace
    pc, query = re.search(r"\(pc=0x([0-9a-f]+), query=0x([0-9a-f]+)\)", trace).groups()
    assert int(pc, 16) - int(query, 16) == 1, trace
