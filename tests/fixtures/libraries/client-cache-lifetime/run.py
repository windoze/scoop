import subprocess
import sys

cases = [
    (stage, mode, cleanup)
    for stage in ("lookup", "begin", "write", "commit")
    for mode, cleanup in (
        ("failure", "plain"),
        ("caller", "plain"),
        ("client", "plain"),
        ("client", "cleanup"),
    )
] + [(stage, "total", "plain") for stage in ("lookup", "commit")]

for case in cases:
    result = subprocess.run([sys.argv[1], *case], capture_output=True, text=True, timeout=180)
    assert result.returncode == 0, (case, result.returncode, result.stdout, result.stderr)
    assert result.stdout == "cache lifetime case passed\n" and not result.stderr, (
        case,
        result.stdout,
        result.stderr,
    )

print("client cache callback cancellation cleanup and failure classification passed")
