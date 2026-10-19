import gzip

from wire import chunked, closed, exact, request, response


def incoming(connection, method, path):
    first, fields = request(connection)
    assert first == method + b" " + path + b" HTTP/1.1", first
    return fields


def replay(listener):
    for attempt in (1, 2):
        with listener.accept()[0] as connection:
            connection.settimeout(45)
            if attempt == 1:
                incoming(connection, b"GET", b"/warm")
                response(connection)
            fields = incoming(connection, b"PUT", b"/start")
            assert fields[b"expect"].lower() == b"100-continue"
            assert fields[b"content-encoding"] == b"gzip"
            assert fields[b"transfer-encoding"] == b"chunked"
            assert fields[b"trailer"].lower() == b"x-attempt"
            connection.sendall(b"HTTP/1.1 100 Continue\r\n\r\n")
            body, trailers = chunked(connection)
            assert gzip.decompress(body) == b"payload"
            assert trailers == [(b"x-attempt", str(attempt).encode())]
            if attempt == 2:
                encoded = gzip.compress(b"done", mtime=0)
                connection.sendall(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n"
                    b"Content-Encoding: gzip\r\nTrailer: X-Final\r\n\r\n"
                    + f"{len(encoded):x}\r\n".encode()
                    + encoded
                    + b"\r\n0\r\nX-Final: gzip\r\n\r\n"
                )
                closed(connection)


def status(listener):
    with listener.accept()[0] as connection:
        connection.settimeout(45)
        incoming(connection, b"POST", b"/start")
        connection.sendall(
            b"HTTP/1.1 503 Busy\r\nContent-Length: 100\r\nContent-Encoding: gzip\r\n\r\n"
        )
        closed(connection)
    with listener.accept()[0] as connection:
        connection.settimeout(45)
        incoming(connection, b"POST", b"/start")
        connection.sendall(b"HTTP/1.1 302 Move\r\nLocation: /after\r\nContent-Length: 0\r\n\r\n")
        incoming(connection, b"GET", b"/after")
        response(connection, 503, b"")
        incoming(connection, b"GET", b"/after")
        response(connection, body=b"done")
        closed(connection)


def post(listener):
    with listener.accept()[0] as connection:
        connection.settimeout(45)
        fields = incoming(connection, b"POST", b"/start")
        assert fields[b"content-length"] == b"4"
        assert exact(connection, 4) == b"once"


def total(listener):
    with listener.accept()[0] as connection:
        connection.settimeout(45)
        fields = incoming(connection, b"PUT", b"/start")
        assert fields[b"content-length"] == b"4"
        assert exact(connection, 4) == b"sent"
        connection.sendall(b"HTTP/1.1 503 Busy\r\nRetry-After: 60\r\nContent-Length: 0\r\n\r\n")
        closed(connection)
