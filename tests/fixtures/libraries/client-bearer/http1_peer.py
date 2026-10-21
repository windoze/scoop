import gzip

from wire import chunked, closed, request


def incoming(connection, method, path, authorization=None, cookie=False):
    connection.settimeout(45)
    first, fields = request(connection)
    assert first == method + b" " + path + b" HTTP/1.1", first
    assert fields.get(b"authorization") == authorization, fields
    assert (b"cookie" in fields) == cookie, fields
    return fields


def response(connection, status, extra=b"", body=b""):
    connection.sendall(
        b"HTTP/1.1 "
        + str(status).encode()
        + b" Actual response\r\nConnection: close\r\nContent-Length: "
        + str(len(body)).encode()
        + b"\r\n"
        + extra
        + b"\r\n"
        + body
    )


def challenge(first, _second):
    with first.accept()[0] as connection:
        fields = incoming(connection, b"POST", b"/private/start")
        assert fields[b"expect"].lower() == b"100-continue"
        connection.sendall(
            b"HTTP/1.1 103 Early Hints\r\nLink: </example>\r\n\r\n"
            b"HTTP/1.1 401 Challenge\r\nConnection: close\r\n"
            b'WWW-Authenticate: Other token==, Basic realm="skip"\r\n'
            b'WWW-Authenticate: Bearer realm="private", error="invalid_token"\r\n'
            b"Content-Encoding: gzip\r\nContent-Length: 100\r\n\r\n"
        )
        closed(connection)

    expected = b"bearer opaque.-_~+/=="
    with first.accept()[0] as connection:
        fields = incoming(connection, b"POST", b"/private/start", expected)
        assert fields[b"expect"].lower() == b"100-continue"
        assert fields[b"content-encoding"] == b"gzip"
        assert fields[b"trailer"].lower() == b"x-upload"
        connection.sendall(b"HTTP/1.1 100 Continue\r\n\r\n")
        body, trailers = chunked(connection)
        assert gzip.decompress(body) == b"payload"
        assert trailers == [(b"x-upload", b"once")]
        encoded = gzip.compress(b"done", mtime=0)
        connection.sendall(
            b"HTTP/1.1 200 OK\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n"
            b"Content-Encoding: gzip\r\nTrailer: X-Final\r\n\r\n"
            + f"{len(encoded):x}\r\n".encode()
            + encoded
            + b"\r\n0\r\nX-Final: gzip\r\n\r\n"
        )
        closed(connection)


def redirect(first, second):
    for method, path, authorization, status, extra in (
        (
            b"POST",
            b"/private/start",
            b"bearer old",
            401,
            b'WWW-Authenticate: Bearer error="invalid_token"\r\n',
        ),
        (b"POST", b"/private/start", b"bearer new", 302, b"Location: sibling\r\n"),
        (b"GET", b"/private/sibling", b"bearer new", 302, b"Location: /outside\r\n"),
        (
            b"GET",
            b"/outside",
            b"bearer new",
            302,
            f"Location: http://127.0.0.1:{second.getsockname()[1]}/target\r\n".encode(),
        ),
    ):
        with first.accept()[0] as connection:
            fields = incoming(connection, method, path, authorization, cookie=True)
            assert fields[b"x-keep"] == b"same"
            response(connection, status, extra)
            closed(connection)
    with second.accept()[0] as connection:
        fields = incoming(connection, b"GET", b"/target", b"bearer second")
        assert fields[b"x-keep"] == b"same"
        response(connection, 200, body=b"done")
        closed(connection)
