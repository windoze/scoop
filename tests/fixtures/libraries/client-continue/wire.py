import struct


def exact(connection, count):
    result = bytearray()
    while len(result) < count:
        part = connection.recv(count - len(result))
        assert part, "unexpected peer EOF"
        result.extend(part)
    return bytes(result)


def request_head(connection):
    header = bytearray()
    while not header.endswith(b"\r\n\r\n"):
        header.extend(exact(connection, 1))
        assert len(header) <= 8192
    lines = bytes(header).split(b"\r\n")
    fields = {}
    for line in lines[1:-2]:
        name, value = line.split(b":", 1)
        assert name.lower() not in fields
        fields[name.lower()] = value.strip()
    return lines[0], fields


def h1_response(connection, status=200, body=b"ok"):
    connection.sendall(
        b"HTTP/1.1 "
        + str(status).encode()
        + b" Result\r\nContent-Length: "
        + str(len(body)).encode()
        + b"\r\n\r\n"
        + body
    )


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


def control(connection, kind, flags, stream, payload):
    assert kind in (4, 8), (kind, flags, stream, payload)
    if kind == 4 and not flags & 1:
        send_frame(connection, 4, 1, 0)


def h2_request(connection):
    while True:
        kind, flags, stream, payload = frame(connection)
        if kind == 1:
            assert flags & 4 and not flags & 1
            return stream
        control(connection, kind, flags, stream, payload)


def h2_body(connection, expected_stream):
    body = bytearray()
    while True:
        kind, flags, stream, payload = frame(connection)
        if kind == 0:
            assert stream == expected_stream, "another stream sent body before its Continue"
            body.extend(payload)
            if flags & 1:
                return bytes(body)
        elif kind == 1:
            assert stream == expected_stream
            assert flags == 5 and payload == b"", "expected empty request trailers"
            return bytes(body)
        else:
            control(connection, kind, flags, stream, payload)


def h2_response(connection, stream):
    send_frame(connection, 1, 4, stream, b"\x88")
    send_frame(connection, 0, 1, stream, b"ok")


def closed(connection, h2=False):
    while True:
        try:
            first = connection.recv(1)
        except ConnectionResetError:
            return
        if not first:
            return
        assert h2, "request body was sent after rejection or cancellation"
        header = first + exact(connection, 8)
        payload = exact(connection, int.from_bytes(header[:3], "big"))
        assert header[3] in (3, 4, 7, 8), (header, payload)
