"""Control real standard streams to verify console output and best-effort recovery."""

import os
import subprocess
import sys
from collections import Counter


def run(executable, mode, **kwargs):
    return subprocess.run(
        [executable, mode],
        capture_output=True,
        timeout=30,
        check=False,
        **kwargs,
    )


def ordinary_output(executable):
    result = run(executable, "output")
    assert result.returncode == 0, result
    assert (
        result.stdout
        == ("1970-01-01T00:00:00Z [INFO] app: stdout 雪😀\0\n\ntrailing\n\ncontinued\n").encode()
    ), result.stdout
    assert result.stderr == (
        b"1970-01-01T00:00:00Z [WARN] app: first\nsecond | cause: original\n"
        b"1970-01-01T00:00:00Z [ERROR] app: [log message failed] message failure\n"
    ), result.stderr


def recovery_output(executable):
    result = run(executable, "failures")
    assert result.returncode == 0, result
    assert result.stdout == b"partial\ncontinued\n", result.stdout
    prefix = "[log fallback] "
    source = " target=app source=fixture.scoop:17:9"
    expected = [
        "phase=isEnabled error=query failure level=INFO" + source,
        "phase=isEnabled error=query failure level=WARN target=app",
        "phase=emit error=write failure level=ERROR"
        + source
        + " message=original text cause=original cause",
        "phase=emit error=write failure level=WARN"
        + source
        + " message=[log message failed] message failure cause=business cause"
        + " messageFailure=message failure",
        "phase=flush error=flush failure",
        "phase=emit error=write failure level=INFO" + source + " message=partly written",
        "phase=emit error=format failure level=INFO" + source + " message=format input",
        "phase=emit error=clock failure level=INFO" + source + " message=clock input",
    ]
    assert result.stderr.decode().splitlines() == [prefix + line for line in expected], (
        result.stderr
    )


def failed_descriptors(executable):
    for descriptor, mode in [(1, "stdout-failure"), (2, "stderr-failure")]:
        result = run(executable, mode, preexec_fn=lambda fd=descriptor: os.close(fd))
        assert result.returncode == 17, result
        if descriptor == 1:
            assert result.stdout == b"", result.stdout
            assert result.stderr.count(b"[log fallback]") == 1, result.stderr
            assert b"phase=emit" in result.stderr and b"message=pipe input" in result.stderr
        else:
            assert result.stdout == b"continued\n", result.stdout
            assert result.stderr == b"", result.stderr

    for descriptor, mode in [(1, "stdout-failure"), (2, "stderr-failure")]:
        read_end, write_end = os.pipe()
        os.close(read_end)
        kwargs = {"stdout": subprocess.PIPE, "stderr": subprocess.PIPE}
        kwargs["stdout" if descriptor == 1 else "stderr"] = write_end
        try:
            result = subprocess.run([executable, mode], timeout=30, check=False, **kwargs)
        finally:
            os.close(write_end)
        assert result.returncode == 17, result
        if descriptor == 1:
            assert result.stderr.count(b"[log fallback]") == 1, result.stderr
            assert b"message=pipe input" in result.stderr, result.stderr
        else:
            assert result.stdout == b"continued\n", result.stdout

    def close_both():
        os.close(1)
        os.close(2)

    result = run(executable, "stdout-failure", preexec_fn=close_both)
    assert result.returncode == 17 and result.stdout == result.stderr == b"", result


def concurrent_output(executable):
    result = run(executable, "concurrent")
    assert result.returncode == 0 and not result.stderr, result
    expected = [
        f"{worker}:{index}:".encode() + b"x" * 12000 for worker in range(4) for index in range(20)
    ]
    assert Counter(result.stdout.splitlines()) == Counter(expected)


ordinary_output(sys.argv[1])
recovery_output(sys.argv[1])
failed_descriptors(sys.argv[1])
concurrent_output(sys.argv[1])
print("log streams, fallback, closed descriptors, broken pipes and concurrent records passed")
