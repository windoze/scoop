import struct
import subprocess
import sys
from collections import Counter


def string(value):
    size = len(value)
    prefix = bytearray([min(size, 127)])
    if size >= 127:
        size -= 127
        while size >= 128:
            prefix.append(size % 128 + 128)
            size //= 128
        prefix.append(size)
    return bytes(prefix) + value


def fields(values):
    return b"".join(b"\0" + string(name) + string(value) for name, value in values)


def request(path):
    return [
        (b":method", b"GET"),
        (b":scheme", b"https"),
        (b":authority", b"example.test"),
        (b":path", path),
    ]


def scenario(large):
    wire = bytearray(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
    spans = []

    def frame(kind, flags, stream, payload, message=False):
        start = len(wire)
        wire.extend(len(payload).to_bytes(3, "big"))
        wire.extend(bytes([kind, flags]) + struct.pack("!I", stream) + payload)
        if message:
            spans.append((start, len(wire), stream))

    frame(4, 0, 0, b"")
    block = fields(request(b"/first") + [(b"x-large", b"z" * (40000 if large else 73))])
    frame(1, 40, 1, b"\x02" + b"\0" * 5 + block[:17] + b"\0\0", True)
    for start in range(17, len(block), 8191):
        end = min(start + 8191, len(block))
        frame(9, 4 if end == len(block) else 0, 1, block[start:end], True)
    frame(6, 0, 0, b"12345678")
    frame(99, 0, 0, b"ignored extension")
    frame(1, 4, 3, fields(request(b"/second")), True)
    frame(0, 8, 1, b"\x02alpha\0\0", True)
    frame(0, 1, 3, b"omega", True)
    frame(1, 5, 1, fields([(b"x-end", b"done")]), True)
    frame(3, 0, 3, struct.pack("!I", 8))
    frame(0, 0, 3, b"ignored after reset")
    return bytes(wire), spans


def check_trace(lines, wire, spans, capacity):
    cursor = 0
    for line in lines:
        start, count, encoded = line.split(":")
        start, count = int(start), int(count)
        assert start == cursor and count == min(capacity, len(wire) - cursor)
        end = start + count
        actual = Counter()
        for item in filter(None, encoded.split(";")):
            stream, amount = map(int, item.split(","))
            assert stream in (1, 3) and amount > 0
            actual[stream] += amount
        expected = Counter()
        for first, last, stream in spans:
            header_end = first + 9
            if start < header_end <= end:
                expected[stream] += 9
            payload = max(0, min(end, last) - max(start, header_end))
            if payload:
                expected[stream] += payload
        assert actual == expected, (start, count, actual, expected)
        cursor = end
    assert cursor == len(wire)


for mode, capacity in (("tiny", 1), ("short", 7), ("large", 4093)):
    wire, spans = scenario(mode == "large")
    result = subprocess.run(
        [sys.argv[1], wire.hex(), mode],
        capture_output=True,
        text=True,
        timeout=180,
    )
    assert result.returncode == 0 and not result.stderr, (mode, result.returncode, result.stderr)
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
        check_trace(trace, wire, spans, capacity)
print("HTTP2 input progress matches independent wire framing")
