import os
import subprocess
import sys


def consume(binary, *args):
    result = subprocess.run(
        [binary, *args],
        env=os.environ,
        capture_output=True,
        text=True,
        timeout=180,
    )
    assert result.returncode == 0, (args, result.returncode, result.stdout, result.stderr)
    assert result.stdout == f"client Bearer {args[0]}\n" and not result.stderr, result


def main():
    consume(sys.argv[1], "clear-shared")
    consume(sys.argv[1], "failure", "direct")
    consume(sys.argv[1], "failure", "nested")
    consume(sys.argv[1], "self")
    for case in ("caller", "total"):
        consume(sys.argv[1], "shared", case)
    for case in ("caller", "total", "clear", "client"):
        for cleanup in ("none", "plain", "wrapped"):
            consume(sys.argv[1], "cancel", case, cleanup)
    print("shared Bearer failures cleanup and cancellation passed")


if __name__ == "__main__":
    main()
