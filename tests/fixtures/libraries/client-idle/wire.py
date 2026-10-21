import struct


def exact(connection, count):
    result = bytearray()
    while len(result) < count:
        part = connection.recv(count - len(result))
        assert part, "unexpected peer EOF"
        result.extend(part)
    return bytes(result)


def encoded(kind, flags, stream, payload=b""):
    return (
        len(payload).to_bytes(3, "big") + bytes((kind, flags)) + struct.pack("!I", stream) + payload
    )


def frame(connection):
    header = exact(connection, 9)
    length = int.from_bytes(header[:3], "big")
    assert length <= 65536
    return header[3], header[4], int.from_bytes(header[5:], "big"), exact(connection, length)


def begin(connection, zero_window=False):
    assert exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    kind, flags, stream, _ = frame(connection)
    assert kind == 4 and flags == 0 and stream == 0
    payload = struct.pack("!HI", 4, 0) if zero_window else b""
    connection.sendall(encoded(4, 0, 0, payload) + encoded(4, 1, 0))


def h2_request(connection, expected, end=True):
    headers = False
    while True:
        kind, flags, stream, payload = frame(connection)
        if kind == 1:
            assert stream == expected and flags & 4, (stream, expected, flags)
            if headers:
                assert flags & 1 and not payload
                return
            headers = True
            if not end:
                assert not flags & 1
                return
            if flags & 1:
                return
        elif kind == 0:
            assert headers and stream == expected and not payload, (stream, expected, payload)
            if flags & 1:
                return
        else:
            assert kind in (4, 6, 8), (kind, flags, stream, payload)


def h1_request(connection, expected):
    header = bytearray()
    while not header.endswith(b"\r\n\r\n"):
        header.extend(exact(connection, 1))
        assert len(header) <= 8192
    lines = bytes(header).split(b"\r\n")
    assert lines[0] == expected
    fields = {}
    for line in lines[1:-2]:
        name, value = line.split(b":", 1)
        assert name.lower() not in fields
        fields[name.lower()] = value.strip()
    return fields


def h2_requests(connection, expected):
    headers = set()
    ended = set()
    while ended != expected:
        kind, flags, stream, payload = frame(connection)
        if kind == 1:
            assert stream in expected and stream not in ended and flags & 4
            if stream in headers:
                assert flags & 1 and not payload
            headers.add(stream)
            if flags & 1:
                ended.add(stream)
        elif kind == 0:
            assert stream in headers and stream not in ended and not payload
            if flags & 1:
                ended.add(stream)
        else:
            assert kind in (4, 6, 8), (kind, flags, stream, payload)
    assert headers == expected


def closed(connection, h2=False):
    while True:
        try:
            first = connection.recv(1)
        except ConnectionResetError:
            return
        if not first:
            return
        assert h2, "unexpected HTTP/1 output after the request"
        header = first + exact(connection, 8)
        payload = exact(connection, int.from_bytes(header[:3], "big"))
        assert header[3] in (3, 4, 6, 7, 8), (header, payload)
