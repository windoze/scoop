import os
import subprocess
import sys


def consume(binary, *args):
    result = subprocess.run(
        [binary, *args],
        env=os.environ,
        capture_output=True,
        text=True,
        timeout=600,
    )
    assert result.returncode == 0, (args, result.returncode, result.stdout, result.stderr)
    assert result.stdout == f"client proxy selector {args[0]}\n" and not result.stderr, result


def main():
    consume(sys.argv[1], "suspend")
    for kind in ("direct", "nested"):
        consume(sys.argv[1], "failure", kind)
    for case in ("caller", "total", "client"):
        for cleanup in ("none", "plain"):
            consume(sys.argv[1], "cancel", case, cleanup)
    consume(sys.argv[1], "cancel", "caller", "wrapped")
    print("suspend proxy selection Context cancellation and cleanup passed")


if __name__ == "__main__":
    main()
