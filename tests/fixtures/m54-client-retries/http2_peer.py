import struct

from wire import exact, send_frame


def frame(connection, first=None):
    header = exact(connection, 9) if first is None else first + exact(connection, 8)
    length = int.from_bytes(header[:3], "big")
    assert length <= 65536
    return header[3], header[4], int.from_bytes(header[5:], "big"), exact(connection, length)


def settings(connection, kind, flags, stream, payload):
    if kind != 4:
        return False
    assert stream == 0 and len(payload) % 6 == 0
    if flags & 1:
        assert not payload
    else:
        send_frame(connection, 4, 1, 0)
    return True


def begin(connection):
    connection.settimeout(45)
    assert exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    send_frame(connection, 4, 0, 0)
    while True:
        kind, flags, stream, payload = frame(connection)
        if settings(connection, kind, flags, stream, payload):
            if not flags & 1:
                return
        else:
            assert kind == 8


def request(connection, expected, previous=()):
    while True:
        kind, flags, stream, payload = frame(connection)
        if settings(connection, kind, flags, stream, payload) or kind == 8:
            continue
        if kind == 3:
            assert stream in previous and payload == struct.pack("!I", 8)
        elif kind == 1 and stream in previous:
            assert flags & 5 == 5 and not payload
        else:
            assert kind == 1 and flags & 4 and stream == expected and payload
            return


def body(connection, expected):
    result = bytearray()
    while True:
        kind, flags, stream, payload = frame(connection)
        if settings(connection, kind, flags, stream, payload) or kind == 8:
            continue
        assert stream == expected
        if kind == 0:
            result.extend(payload)
        else:
            assert kind == 1 and flags & 4 and not payload
        if flags & 1:
            return bytes(result)


def close(connection, pending_request=False):
    while True:
        try:
            first = connection.recv(1)
        except ConnectionResetError:
            return
        if not first:
            return
        kind, flags, stream, payload = frame(connection, first)
        if kind == 1:
            assert stream == 1 and flags & 4
            assert pending_request or (flags & 1 and not payload)
        elif kind == 0:
            assert stream == 1 and not payload
        else:
            assert kind in (3, 4, 7, 8)


def respond(connection, stream):
    send_frame(connection, 1, 4, stream, b"\x88")
    send_frame(connection, 0, 1, stream, b"done")


def refused(listener):
    with listener.accept()[0] as connection:
        begin(connection)
        request(connection, 1)
        send_frame(connection, 3, 0, 1, struct.pack("!I", 7))
        request(connection, 3, (1,))
        send_frame(connection, 1, 4, 3, b"\x08\x03100")
        assert body(connection, 3) == b"once"
        respond(connection, 3)
        close(connection)


def goaway(listener):
    with listener.accept()[0] as connection:
        begin(connection)
        send_frame(connection, 7, 0, 0, struct.pack("!II", 0, 0))
        close(connection, pending_request=True)
    with listener.accept()[0] as connection:
        begin(connection)
        request(connection, 1)
        respond(connection, 1)
        close(connection)
