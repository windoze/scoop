"""Check the selected minimum OS in real Scoop, C, runtime and executable objects."""

import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import cbor2


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


def check_archive(path, version, zstd):
    data = path.read_bytes()
    assert data[:8] == b"SCOOPSLB"
    offset = 16 + struct.unpack_from("<Q", data, 8)[0]
    manifest = cbor2.loads(decompress(data[16:offset], zstd))
    objects = []
    dictionary = None
    for record in manifest[8]:
        size = record[4]
        payload = data[offset : offset + size]
        assert len(payload) == size
        role = record[3][0]
        if role in (4, 7):
            objects.append((role, payload))
        elif role == 8:
            assert dictionary is None
            dictionary = payload
        offset += size
    assert offset == len(data)
    with tempfile.TemporaryDirectory(dir=path.parent) as temporary:
        dictionary_args = []
        if dictionary is not None:
            dictionary_path = Path(temporary) / "objects.dict"
            dictionary_path.write_bytes(dictionary)
            dictionary_args = ["-D", str(dictionary_path)]
        for _, payload in objects:
            assert check_object(decompress(payload, zstd, dictionary_args), version)
    assert {4, 7} <= {role for role, _ in objects}, "fixture must include Scoop and Native objects"


def decompress(payload, zstd, dictionary_args=()):
    return subprocess.run(
        [zstd, "--decompress", "--stdout", "--quiet", *dictionary_args],
        input=payload,
        capture_output=True,
        check=True,
    ).stdout


def main():
    parts = [int(part) for part in sys.argv[1].split(".")]
    parts += [0] * (3 - len(parts))
    expected = parts[0] << 16 | parts[1] << 8 | parts[2]
    check_archive(Path(sys.argv[2]), expected, sys.argv[5])
    assert check_object(Path(sys.argv[3]).read_bytes(), expected)
    runtime = list((Path(sys.argv[4]).parent / "objects").glob("*.o"))
    assert runtime
    for path in runtime:
        assert check_object(path.read_bytes(), expected), path
    print("deployment matches")


if __name__ == "__main__":
    main()
