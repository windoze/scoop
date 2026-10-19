import os
import subprocess
import sys

for group in ("callback", "callback-cleanup", "callback-wrapped", "prehash", "prehash-cleanup"):
    for case in (
        ("success", "caller", "total", "client")
        if group.startswith("prehash")
        else ("caller", "total", "client")
    ):
        result = subprocess.run(
            [sys.argv[1], group, case],
            env=dict(os.environ, SCOOP_GC_STRESS_MOVE="1"),
            capture_output=True,
            text=True,
            timeout=180,
        )
        assert result.returncode == 0 and not result.stderr, (group, case, result)
        assert result.stdout == f"Digest lifetime {group} {case}\n", result
print("Digest callbacks and prehash wait for cancellation and actual cleanup")
