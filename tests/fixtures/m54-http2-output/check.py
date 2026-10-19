import os
import struct
import subprocess
import sys
from collections import Counter


def check_trace(lines):
    chunks = []
    for line in lines:
        kind, stream, count, encoded = line.split(":")
        payload = bytes.fromhex(encoded)
        assert len(payload) == int(count) > 0
        chunks.append((int(kind), int(stream), payload))
    wire = b"".join(part for _, _, part in chunks)
    assert wire[:24] == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    spans = [(0, 24, 0, 0)]
    headers = Counter()
    data = {1: bytearray(), 3: bytearray()}
    empty_end = reset = continuation = False
    offset = 24
    while offset < len(wire):
        assert offset + 9 <= len(wire)
        size = int.from_bytes(wire[offset : offset + 3], "big")
        kind, flags = wire[offset + 3 : offset + 5]
        stream = int.from_bytes(wire[offset + 5 : offset + 9], "big") & 0x7FFFFFFF
        end = offset + 9 + size
        assert end <= len(wire)
        payload = wire[offset + 9 : end]
        category = 2 if kind == 0 else 1 if kind in (1, 9) else 0
        spans.append((offset, end, category, stream))
        if kind == 0:
            assert stream in data
            data[stream] += payload
            if stream == 3 and not payload and flags & 1:
                empty_end = True
        elif kind in (1, 9):
            assert stream in data
            if kind == 1:
                headers[stream] += 1
            else:
                assert stream == 1
                continuation = True
        elif kind == 3:
            assert stream == 3 and payload == struct.pack("!I", 8)
            reset = True
        else:
            assert kind == 4 and stream == 0
        offset = end
    assert headers == {1: 2, 3: 1}
    assert data == {1: b"first", 3: b"two"}
    assert empty_end and reset and continuation
    offset = index = 0
    for kind, stream, payload in chunks:
        end = offset + len(payload)
        while offset < end:
            start, stop, expected_kind, expected_stream = spans[index]
            assert start <= offset < stop
            assert (kind, stream) == (expected_kind, expected_stream), (
                offset,
                kind,
                stream,
                expected_kind,
                expected_stream,
            )
            offset = min(end, stop)
            if offset == stop:
                index += 1
    assert index == len(spans) and offset == len(wire)


result = subprocess.run(
    [sys.argv[1]],
    env=dict(os.environ, SCOOP_GC_STRESS_MOVE="1"),
    capture_output=True,
    text=True,
    timeout=180,
)
assert result.returncode == 0 and not result.stderr, (result.returncode, result.stderr)
traces = []
current = None
for line in result.stdout.splitlines():
    if line == "begin":
        assert current is None
        current = []
    elif line == "end":
        assert current is not None
        traces.append(current)
        current = None
    else:
        assert current is not None
        current.append(line)
assert current is None and len(traces) == 2
for trace in traces:
    check_trace(trace)
print("HTTP2 output metadata matches independent wire framing")
