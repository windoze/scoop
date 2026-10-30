"""Execute the target with raw POSIX argument and environment bytes."""

import os
import sys

program, temporary = map(os.fsencode, sys.argv[1:])
environment = {
    b"EMPTY": b"",
    b"PAIR": b"a=b",
    b"K\xff": b"V\xfe",
    b"HOME": b"/h\xff",
    b"TMPDIR": temporary,
}
if b"SCOOP_GC_STRESS_MOVE" in os.environb:
    environment[b"SCOOP_GC_STRESS_MOVE"] = b"1"
os.execve(program, [program, b"", b"two words", b"-\xff", "中".encode()], environment)
