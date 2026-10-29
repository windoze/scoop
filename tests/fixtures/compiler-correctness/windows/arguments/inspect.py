"""Exercise native Windows argv, including argv[0], through real startup and CLI run."""

import json
import subprocess
import sys


def canonical_wtf8(text):
    return (
        text.encode("utf-16-le", "surrogatepass")
        .decode("utf-16-le", "surrogatepass")
        .encode("utf-8", "surrogatepass")
    )


def raw_output(arguments):
    lines = [str(len(arguments))]
    lines.extend(",".join(str(byte) for byte in canonical_wtf8(argument)) for argument in arguments)
    return ("\n".join(lines) + "\n").encode("ascii")


def launch(arguments, executable=None):
    return subprocess.run(
        arguments, executable=executable, capture_output=True, timeout=60, check=False
    )


mode, program, *extra = sys.argv[1:]
values = [
    "",
    "值🙂",
    "\x7f\x80\u07ff\u0800\ud7ff\uffff",
    "high-\ud800-low-\udc00",
    "\ud800\ud800\udc00\udc00",
    "\udbff\udfff",
    'quoted "value"',
    "end\\",
]

if mode == "raw":
    for name in ("../literal app name", "raw-\ud800-\udc00", program):
        arguments = [name, *values]
        result = launch(arguments, executable=program)
        assert result.returncode == 0, (result.returncode, result.stderr)
        assert result.stdout == raw_output(arguments), result.stdout
        assert result.stderr == b"", result.stderr
elif mode == "typed":
    arguments = ["../literal app name", "two words", "", "值🙂", 'quoted "value"', "end\\"]
    result = launch(arguments, executable=program)
    assert result.returncode == 0, (result.returncode, result.stderr)
    assert result.stdout == ("typed\n" + "\n".join(arguments) + "\n").encode("utf-8")
    assert result.stderr == b"", result.stderr
    for bad in ("\ud800", "\udc00", "x\ud800x", "\ud800\ud800\udc00"):
        for index in (0, 1):
            arguments = ["typed", "valid"]
            arguments[index] = bad
            result = launch(arguments, executable=program)
            assert result.returncode == 1, (result.returncode, result.stderr)
            assert result.stdout == b"", result.stdout
            expected = (
                f"invalid UTF-8 in argv[{index}]\n"
                "scoop: uncaught exception: n(c=scoop%3Ascoop.core%3A0.1.0;"
                "p=;o=-;k=C;x=CharacterCodingException)\n"
                "  <stack trace unavailable: disabled>\n"
            ).encode("ascii")
            assert result.stderr == expected, result.stderr
elif mode == "cli":
    output, *build_arguments = extra
    result = launch([program, "run", *build_arguments, "--", *values])
    assert result.returncode == 0, (result.returncode, result.stderr)
    assert result.stdout == raw_output([output, *values]), result.stdout
    for line in result.stderr.splitlines():
        json.loads(line)
else:
    raise ValueError(mode)

sys.stdout.buffer.write((mode + " passed\n").encode("ascii"))
