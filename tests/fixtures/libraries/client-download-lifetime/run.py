import subprocess
import sys
from pathlib import Path

cases = [
    ("cancel", last, mode, cleanup)
    for last in ("false", "true")
    for mode in ("caller", "client")
    for cleanup in ("none", "direct", "wrapped", "nested")
] + [("cancel", last, "total", "wrapped") for last in ("false", "true")]
cases += [("callback", nested) for nested in ("false", "true")]
cases += [("self-close",), ("file-error",), ("invalid-path",)]

for case in cases:
    directory = Path(sys.argv[2]) / "-".join(case)
    result = subprocess.run(
        [sys.argv[1], str(directory), *case], capture_output=True, text=True, timeout=180
    )
    assert result.returncode == 0, (case, result.returncode, result.stdout, result.stderr)
    assert result.stdout == "download lifetime case passed\n" and not result.stderr, (
        case,
        result.stdout,
        result.stderr,
    )

print("client download cancellation callbacks cleanup and close ownership passed")
