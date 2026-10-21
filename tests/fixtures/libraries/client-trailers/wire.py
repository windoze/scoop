import struct


def exact(connection, count):
    result = bytearray()
    while len(result) < count:
        part = connection.recv(count - len(result))
        assert part, "unexpected peer EOF"
        result.extend(part)
    return bytes(result)


def line(connection):
    result = bytearray()
    while not result.endswith(b"\r\n"):
        result.extend(exact(connection, 1))
        assert len(result) <= 8192
    return bytes(result[:-2])


def fields(connection):
    result = []
    while value := line(connection):
        name, value = value.split(b":", 1)
        result.append((name.lower(), value.strip()))
    return result


def request(connection):
    first = line(connection)
    headers = fields(connection)
    result = dict(headers)
    assert len(result) == len(headers)
    return first, result


def chunked(connection):
    result = bytearray()
    while True:
        count = int(line(connection), 16)
        assert 0 <= count <= 65536
        if count == 0:
            return bytes(result), fields(connection)
        result.extend(exact(connection, count))
        assert exact(connection, 2) == b"\r\n"


def send_frame(connection, kind, flags, stream, payload=b""):
    connection.sendall(
        len(payload).to_bytes(3, "big") + bytes((kind, flags)) + struct.pack("!I", stream) + payload
    )


def upgrade(connection):
    connection.sendall(
        b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: h2c\r\n\r\n"
    )
    send_frame(connection, 4, 0, 0)
    assert exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    while True:
        header = exact(connection, 9)
        count = int.from_bytes(header[:3], "big")
        assert count <= 65536 and header[3] in (4, 8)
        exact(connection, count)
        if header[3] == 4 and not header[4] & 1:
            send_frame(connection, 4, 1, 0)
            break
    send_frame(connection, 1, 4, 1, b"\x88")
    send_frame(connection, 0, 1, 1, b"ok")


def response(connection, status=200, body=b"ok"):
    connection.sendall(
        b"HTTP/1.1 "
        + str(status).encode()
        + b" Result\r\nContent-Length: "
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
        assert h2, "body or trailer was sent after final response"
        header = first + exact(connection, 8)
        count = int.from_bytes(header[:3], "big")
        assert count <= 65536 and header[3] in (3, 4, 7, 8)
        exact(connection, count)
