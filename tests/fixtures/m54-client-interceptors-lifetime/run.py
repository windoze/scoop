import subprocess
import sys

cases = []
for where in ("before", "after", "attempt"):
    for child in ("false", "true"):
        cases += [
            ("lifetime", where, stop, child, "true") for stop in ("caller", "client", "total")
        ]
        cases += [("lifetime", where, "client", child, "false")]
        cases += [("self-wait", where, child)]
cases += [("concurrent-proceed",), ("http2-shared-driver",)]

for case in cases:
    result = subprocess.run([sys.argv[1], *case], capture_output=True, text=True, timeout=180)
    assert result.returncode == 0, (case, result.returncode, result.stdout, result.stderr)
    assert result.stdout == "interceptor lifetime case passed\n" and not result.stderr, (
        case,
        result.stdout,
        result.stderr,
    )

print("client interceptor cancellation deadlines cleanup self-wait and HTTP2 isolation passed")
