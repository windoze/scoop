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


def request(connection, path, method=b"GET", accept=b"gzip, deflate, br, zstd"):
    assert line(connection) == method + b" " + path + b" HTTP/1.1"
    fields = {}
    while field := line(connection):
        name, value = field.split(b":", 1)
        assert name.lower() not in fields
        fields[name.lower()] = value.strip()
    assert fields.get(b"accept-encoding") == accept
    assert fields.get(b"content-length", b"0") == b"0"
    assert b"transfer-encoding" not in fields
    return fields


def response(connection, body=b"", fields=(), status=200):
    fields = list(fields)
    if not any(name.lower() == b"content-length" for name, _ in fields):
        fields.append((b"Content-Length", str(len(body)).encode()))
    head = b"HTTP/1.1 " + str(status).encode() + b" Result\r\n"
    for name, value in fields:
        head += name + b": " + value + b"\r\n"
    connection.sendall(head + b"\r\n" + body)


def chunked_response(connection, body):
    connection.sendall(
        b"HTTP/1.1 200 Result\r\nContent-Encoding: GZip, , identity\r\n"
        b"content-encoding: deflate\r\nTransfer-Encoding: chunked\r\nTrailer: X-End\r\n\r\n"
    )
    for index in range(0, len(body), 3):
        part = body[index : index + 3]
        connection.sendall(f"{len(part):x}\r\n".encode() + part + b"\r\n")
    connection.sendall(b"0\r\nX-End: verified\r\n\r\n")


def closed(connection):
    try:
        assert not connection.recv(1), "unexpected subsequent request"
    except ConnectionResetError:
        return
