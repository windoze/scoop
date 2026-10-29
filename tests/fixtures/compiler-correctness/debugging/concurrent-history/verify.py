"""Concurrent first throws publish one immutable, GC-managed snapshot."""

import re
import subprocess
import sys

result = subprocess.run([sys.argv[1]], capture_output=True, timeout=30)
assert result.returncode == 0, (result.returncode, result.stderr)
assert result.stdout == b"", result.stdout
trace = result.stderr.decode()
reports = re.findall(r"(?m)^\S[^\n]*\n(?:  [^\n]*\n)*", trace)
assert "".join(reports) == trace and len(reports) == 8, trace
for original, repeated in zip(reports[::2], reports[1::2], strict=True):
    assert original == repeated, (original, repeated)
    assert "x=SharedFailure)" in original.splitlines()[0], original
    assert original.count("  at ") >= 2, original
    assert "<stack not captured" not in original, original
    assert "boundary unavailable" not in original, original
