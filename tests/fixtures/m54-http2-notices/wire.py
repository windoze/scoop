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
    size = int.from_bytes(header[:3], "big")
    assert size <= 65536
    return header[3], header[4], int.from_bytes(header[5:], "big"), exact(connection, size)


def begin(connection, blocked):
    assert exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    kind, flags, stream, _ = frame(connection)
    assert kind == 4 and flags == 0 and stream == 0
    payload = struct.pack("!HI", 4, 0) if blocked else b""
    connection.sendall(encoded(4, 0, 0, payload) + encoded(4, 1, 0))


def request(connection, expected, end=True):
    while True:
        kind, flags, stream, payload = frame(connection)
        if kind == 1:
            assert stream == expected and flags & 4 and bool(flags & 1) == end
            return
        assert kind in (4, 6, 8), (kind, flags, stream, payload)


def closed(connection):
    while True:
        try:
            first = connection.recv(1)
        except ConnectionResetError:
            return
        if not first:
            return
        header = first + exact(connection, 8)
        exact(connection, int.from_bytes(header[:3], "big"))
        assert header[3] in (0, 3, 4, 6, 7, 8), header
