import json
import os
import subprocess
import sys
import time
from pathlib import Path

MODES = (
    "basic-caller",
    "bearer-total",
    "digest-close",
    "failure-basic",
    "failure-bearer",
    "failure-digest",
    "shared-cancel",
    "shared-refresh",
    "shared-refresh-clear",
    "clear-scopes",
    "source",
    "source-failure",
)


def main():
    binary, output = sys.argv[1:]
    timings = []
    for mode in MODES:
        started = time.monotonic()
        result = subprocess.run(
            [binary, mode],
            capture_output=True,
            text=True,
            env=os.environ,
            timeout=600,
        )
        assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
        assert result.stdout == f"proxy authentication lifetime {mode}\n" and not result.stderr, (
            result
        )
        timings.append({"mode": mode, "seconds": round(time.monotonic() - started, 3)})
        Path(output).write_text(json.dumps(timings, indent=2))
    print("proxy authentication suspension cancellation refresh and scoped cache cleanup passed")


if __name__ == "__main__":
    main()
