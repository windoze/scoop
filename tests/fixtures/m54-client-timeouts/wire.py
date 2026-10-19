import struct


def exact(connection, count):
    result = bytearray()
    while len(result) < count:
        part = connection.recv(count - len(result))
        assert part, "unexpected peer EOF"
        result.extend(part)
    return bytes(result)


def request(connection, expected):
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


def response(connection, body):
    connection.sendall(
        b"HTTP/1.1 200 OK\r\nContent-Length: " + str(len(body)).encode() + b"\r\n\r\n" + body
    )


def frame(connection):
    header = exact(connection, 9)
    length = int.from_bytes(header[:3], "big")
    assert length <= 65536
    return header[3], header[4], int.from_bytes(header[5:], "big"), exact(connection, length)


def send_frame(connection, kind, flags, stream, payload=b""):
    connection.sendall(
        len(payload).to_bytes(3, "big") + bytes((kind, flags)) + struct.pack("!I", stream) + payload
    )


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
        assert h2, "unexpected bytes after timeout or completion"
        header = first + exact(connection, 8)
        payload = exact(connection, int.from_bytes(header[:3], "big"))
        assert header[3] in (3, 4, 7, 8), (header, payload)
