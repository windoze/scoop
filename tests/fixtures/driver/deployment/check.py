"""Check the selected minimum OS in real Scoop, C, runtime and executable objects."""

import struct
import sys
from pathlib import Path


def check_object(data, version):
    if data[:4] != b"\xcf\xfa\xed\xfe":
        return False
    count = struct.unpack_from("<I", data, 16)[0]
    offset = 32
    for _ in range(count):
        command, size = struct.unpack_from("<II", data, offset)
        if command == 0x32:
            minimum = struct.unpack_from("<I", data, offset + 12)[0]
            assert minimum == version, (hex(minimum), hex(version))
            return True
        offset += size
    raise AssertionError("Mach-O has no LC_BUILD_VERSION")


def check_archive(path, version):
    data = path.read_bytes()
    assert data[:8] == b"!<arch>\n"
    offset = 8
    count = 0
    while offset < len(data):
        header = data[offset : offset + 60]
        size = int(header[48:58])
        payload = data[offset + 60 : offset + 60 + size]
        if header.startswith(b"#1/"):
            payload = payload[int(header[3:16]) :]
        count += check_object(payload, version)
        offset += 60 + size + size % 2
    assert count >= 2, "fixture must include Scoop and generated/native C objects"


parts = [int(part) for part in sys.argv[1].split(".")]
parts += [0] * (3 - len(parts))
expected = parts[0] << 16 | parts[1] << 8 | parts[2]
check_archive(Path(sys.argv[2]), expected)
assert check_object(Path(sys.argv[3]).read_bytes(), expected)
runtime = list((Path(sys.argv[4]).parent / "objects").glob("*.o"))
assert runtime
for path in runtime:
    assert check_object(path.read_bytes(), expected), path
print("deployment matches")
