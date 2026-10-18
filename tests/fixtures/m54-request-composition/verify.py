import gzip
import sys
import zlib
from email import policy
from email.parser import BytesParser
from pathlib import Path


def verify(path, expected_file):
    encoded = path.read_bytes()
    assert encoded[:3] == b"\x1f\x8b\x08"
    assert encoded[4:8] == b"\0\0\0\0"
    body = gzip.decompress(encoded)
    headers = (
        b'Content-Type: multipart/form-data; boundary="scoop-body-fixture"\r\n'
        b"MIME-Version: 1.0\r\n\r\n"
    )
    message = BytesParser(policy=policy.default).parsebytes(headers + body)
    assert not message.defects
    parts = list(message.iter_parts())
    assert [part.get_param("name", header="content-disposition") for part in parts] == [
        "note",
        "file",
        "coded",
        "empty",
        "nested",
    ]
    assert parts[0].get_payload(decode=True).decode(parts[0].get_content_charset()) == "值 🌍"
    assert parts[1].get_filename() == "report.bin"
    assert parts[1].get_content_type() == "application/octet-stream"
    assert parts[1].get_payload(decode=True) == expected_file
    assert parts[2]["Content-Encoding"] == "deflate"
    decoder = zlib.decompressobj()
    coded = decoder.decompress(parts[2].get_payload(decode=True)) + decoder.flush()
    assert decoder.eof and not decoder.unused_data and not decoder.unconsumed_tail
    assert coded == b"coded part data"
    assert parts[3].get_payload(decode=True) == b""
    assert parts[4].get_content_type() == "multipart/mixed"
    inner = list(parts[4].iter_parts())
    assert len(inner) == 1 and inner[0].get_payload(decode=True) == b"inside"
    return body


first, second, source = map(Path, sys.argv[1:])
assert verify(first, source.read_bytes()) == verify(second, source.read_bytes())
print("Python gzip zlib and MIME verify both generated bodies")
