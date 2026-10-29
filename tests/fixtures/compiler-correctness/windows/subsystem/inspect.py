"""Inspect the actual PE32+ subsystem without running a linker-specific tool."""

import struct
import sys
from pathlib import Path


def main():
    image = Path(sys.argv[1]).read_bytes()
    assert image[:2] == b"MZ", "missing DOS header"
    pe = struct.unpack_from("<I", image, 0x3C)[0]
    assert image[pe : pe + 4] == b"PE\0\0", "missing PE signature"
    optional = pe + 24
    assert struct.unpack_from("<H", image, optional)[0] == 0x20B, "expected PE32+"
    actual = struct.unpack_from("<H", image, optional + 68)[0]
    expected = {"console": 3, "gui": 2}[sys.argv[2]]
    assert actual == expected, (actual, expected)
    sys.stdout.buffer.write((sys.argv[2] + "\n").encode("ascii"))


if __name__ == "__main__":
    main()
