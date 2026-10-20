import contextlib
import urllib.parse

import tls_peer
import wire


def parts(target):
    url = urllib.parse.urlsplit(target)
    authority = url.netloc.rsplit("@", 1)[-1]
    path = (url.path or "/") + ("?" + url.query if url.query else "")
    absolute = urllib.parse.urlunsplit((url.scheme, authority, url.path or "/", url.query, ""))
    return url, authority.encode(), path.encode(), absolute.encode()


@contextlib.contextmanager
def connect(listener, contexts, outer=False):
    with listener.accept()[0] as raw:
        raw.settimeout(180)
        connection = tls_peer.TlsLayer(raw, contexts["http/1.1"]) if outer else raw
        yield connection


def request(connection, target, method=b"POST", forward=True):
    first, fields = wire.request(connection)
    _, authority, path, absolute = parts(target)
    expected = authority if method == b"CONNECT" else absolute if forward else path
    assert first == method + b" " + expected + b" HTTP/1.1", first
    assert fields[b"host"] == authority, fields
    if method == b"CONNECT":
        assert b"authorization" not in fields and b"cookie" not in fields, fields
        assert fields.get(b"content-length", b"0") == b"0", fields
    elif not forward:
        assert b"proxy-authorization" not in fields, fields
    return fields


def entity(connection, fields):
    if fields.get(b"expect", b"").lower() == b"100-continue":
        connection.sendall(b"HTTP/1.1 100 Continue\r\n\r\n")
    if fields.get(b"transfer-encoding", b"").lower() == b"chunked":
        body, trailers = wire.chunked(connection)
        assert not trailers, trailers
        return body
    return wire.exact(connection, int(fields.get(b"content-length", b"0")))


def credentials(fields, expected, name=b"proxy-authorization"):
    if expected is None:
        assert name not in fields, fields
    else:
        scheme, token = fields[name].split(b" ", 1)
        kind, wanted = expected.split(b" ", 1)
        assert scheme.lower() == kind.lower() and token == wanted, fields


def response(connection, body=b"done", extra=b"", status=200):
    connection.sendall(
        b"HTTP/1.1 "
        + str(status).encode()
        + b" Result\r\nConnection: close\r\nContent-Length: "
        + str(len(body)).encode()
        + b"\r\n"
        + extra
        + b"\r\n"
        + body
    )


def challenge(connection, value, *, origin=False, delivered=False):
    status = b"401" if origin else b"407"
    name = b"WWW-Authenticate" if origin else b"Proxy-Authenticate"
    head = b"HTTP/1.1 " + status + b" Challenge\r\n" + name + b": " + value + b"\r\n"
    if delivered:
        connection.sendall(head + b"Connection: close\r\nContent-Length: 4\r\n\r\ndeny")
    else:
        connection.sendall(
            head + b"Transfer-Encoding: chunked\r\nContent-Encoding: unused-coding\r\n\r\n10000\r\n"
        )
    wire.closed(connection)


def tunnel(connection, target, contexts, extra=b"", h2=False):
    connection.sendall(
        b"HTTP/1.1 103 Proxy Hints\r\n\r\nHTTP/1.1 299 Tunnel\r\nContent-Length: 999\r\n"
        + extra
        + b"\r\n"
    )
    assert parts(target)[0].scheme == "https"
    connection = tls_peer.TlsLayer(connection, contexts["h2" if h2 else "http/1.1"])
    assert connection.ssl.selected_alpn_protocol() == ("h2" if h2 else "http/1.1")
    return connection


def http2(connection):
    assert wire.exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    wire.send_frame(connection, 4, 0, 0)
    seen = False
    while True:
        header = wire.exact(connection, 9)
        count, kind, flags = int.from_bytes(header[:3], "big"), header[3], header[4]
        stream = int.from_bytes(header[5:], "big") & 0x7FFFFFFF
        assert count <= 65536
        payload = wire.exact(connection, count)
        if kind == 4:
            if not flags & 1:
                wire.send_frame(connection, 4, 1, 0)
        elif kind == 8:
            pass
        elif kind in (0, 1):
            assert stream == 1
            if not seen:
                assert kind == 1 and payload
                seen = True
            elif kind == 0:
                assert not payload
            if flags & 1:
                break
        else:
            raise AssertionError((kind, flags, stream, payload))
    wire.send_frame(connection, 1, 4, 1, b"\x88")
    wire.send_frame(connection, 0, 1, 1, b"done")
    wire.closed(connection, True)
