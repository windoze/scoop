import base64
import struct


def exact(connection, count):
    result = bytearray()
    while len(result) < count:
        part = connection.recv(count - len(result))
        assert part, "unexpected peer EOF"
        result.extend(part)
    return bytes(result)


def request(connection):
    header = bytearray()
    while not header.endswith(b"\r\n\r\n"):
        header.extend(exact(connection, 1))
        assert len(header) <= 8192
    lines = bytes(header).split(b"\r\n")
    method, target, version = lines[0].split(b" ")
    assert version == b"HTTP/1.1"
    fields = {}
    for line in lines[1:-2]:
        name, value = line.split(b":", 1)
        name = name.lower()
        assert name not in fields
        fields[name] = value.strip()
    assert b"transfer-encoding" not in fields
    return method, target, fields, exact(connection, int(fields.get(b"content-length", b"0")))


def settings(fields):
    assert fields[b"upgrade"] == b"h2c"
    tokens = {part.strip().lower() for part in fields[b"connection"].split(b",")}
    assert {b"upgrade", b"http2-settings"} <= tokens
    value = fields[b"http2-settings"]
    assert b"=" not in value
    payload = base64.urlsafe_b64decode(value + b"=" * (-len(value) % 4))
    values = dict(struct.iter_unpack("!HI", payload))
    assert values[2] == 0 and values[4] == 32
    return values


def frame_bytes(kind, flags, stream, payload=b""):
    return (
        len(payload).to_bytes(3, "big") + bytes((kind, flags)) + struct.pack("!I", stream) + payload
    )


def send_frame(connection, kind, flags, stream, payload=b""):
    connection.sendall(frame_bytes(kind, flags, stream, payload))


def frame(connection):
    header = exact(connection, 9)
    length = int.from_bytes(header[:3], "big")
    assert length <= 65536
    return header[3], header[4], int.from_bytes(header[5:], "big"), exact(connection, length)


def begin_http2(connection, offered):
    assert exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    while True:
        kind, flags, stream, payload = frame(connection)
        assert stream == 0 and kind in (4, 8), (kind, flags, stream)
        if kind == 4 and not flags & 1:
            assert dict(struct.iter_unpack("!HI", payload)) == offered
            send_frame(connection, 4, 1, 0)
            return


def next_request(connection):
    while True:
        kind, flags, stream, payload = frame(connection)
        if kind in (0, 1):
            assert stream != 1, "the upgraded request was sent again"
        if kind == 4:
            assert stream == 0
            if not flags & 1:
                send_frame(connection, 4, 1, 0)
        elif kind == 1:
            assert flags & 4
            return stream
        else:
            assert kind in (0, 8), (kind, flags, stream, payload)


def h1_response(connection, body):
    connection.sendall(
        b"HTTP/1.1 200 No upgrade\r\nContent-Length: "
        + str(len(body)).encode()
        + b"\r\n\r\n"
        + body
    )


def closed(connection, h2=False):
    while True:
        try:
            first = connection.recv(1)
        except ConnectionResetError:
            return
        if not first:
            return
        assert h2, "unexpected request after the response"
        header = first + exact(connection, 8)
        payload = exact(connection, int.from_bytes(header[:3], "big"))
        assert header[3] in (0, 3, 4, 7, 8), (header, payload)


def await_reset(connection):
    while True:
        kind, flags, stream, payload = frame(connection)
        if kind == 3:
            assert stream == 1 and payload == struct.pack("!I", 8)
            return
        assert kind in (0, 4, 8), (kind, flags, stream, payload)
        if kind == 4 and not flags & 1:
            send_frame(connection, 4, 1, 0)
