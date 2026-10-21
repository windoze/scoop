import ipaddress
import urllib.parse

import oracle
import tls_peer
import wire

PROXY = b"Basic cHJveHk6c2VjcmV0"
OTHER_PROXY = b"Basic cHJveHk6b3RoZXI="
SETTINGS = b"\0\0\0\x04\0\0\0\0\0"


def proxy_credentials(actual, expected):
    scheme, token = actual.split(b" ", 1)
    expected_scheme, expected_token = expected.split(b" ", 1)
    assert scheme.lower() == expected_scheme.lower() and token == expected_token, actual


def parts(target):
    url = urllib.parse.urlsplit(target)
    authority = url.netloc.rsplit("@", 1)[-1]
    path = (url.path or "/") + ("?" + url.query if url.query else "")
    absolute = urllib.parse.urlunsplit((url.scheme, authority, url.path or "/", url.query, ""))
    return url, authority.encode(), path.encode(), absolute.encode()


def request(
    connection,
    target,
    *,
    forward=False,
    proxy=PROXY,
    method=b"GET",
    version=b"HTTP/1.1",
    digest=False,
    origin=True,
):
    url, authority, path, absolute = parts(target)
    first, fields = wire.request(connection)
    assert first == method + b" " + (absolute if forward else path) + b" " + version, first
    received = urllib.parse.urlsplit("//" + fields[b"host"].decode())
    assert received.port == url.port, fields
    try:
        assert ipaddress.ip_address(received.hostname) == ipaddress.ip_address(url.hostname)
    except ValueError:
        assert received.hostname == url.hostname, (fields, authority)
    if forward:
        proxy_credentials(fields[b"proxy-authorization"], proxy)
    else:
        assert b"proxy-authorization" not in fields, fields
    if not origin:
        assert b"authorization" not in fields and b"cookie" not in fields, fields
    elif not digest:
        assert fields[b"authorization"] == b"Bearer origin", fields
        assert fields[b"cookie"] == b"origin=cookie", fields
    if method == b"POST":
        assert fields[b"transfer-encoding"] == b"chunked", fields
        body, trailers = wire.chunked(connection)
        assert body == b"abcdef" and not trailers, (body, trailers)
    else:
        assert fields.get(b"content-length", b"0") == b"0", fields
    return fields


def response(connection, body=b"ok", version=b"HTTP/1.1", extra=b""):
    connection.sendall(
        version
        + b" 200 OK\r\nContent-Length: "
        + str(len(body)).encode()
        + b"\r\n"
        + extra
        + b"\r\n"
        + body
    )


def h2(connection, prefixed=False):
    assert wire.exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    if not prefixed:
        connection.sendall(SETTINGS)
    seen = False
    while True:
        head = wire.exact(connection, 9)
        size, kind, flags = int.from_bytes(head[:3], "big"), head[3], head[4]
        stream = int.from_bytes(head[5:], "big") & 0x7FFFFFFF
        assert size <= 65536
        body = wire.exact(connection, size)
        if kind == 4:
            if not flags & 1:
                wire.send_frame(connection, 4, 1, 0)
        elif kind == 8:
            pass
        elif kind in (0, 1):
            assert stream == 1
            if not seen:
                assert kind == 1 and body
                seen = True
            elif kind == 0:
                assert not body
            if flags & 1:
                break
        else:
            raise AssertionError((kind, flags, stream, body))
    wire.send_frame(connection, 1, 4, 1, b"\x88")
    wire.send_frame(connection, 0, 1, 1, b"ok")
    wire.closed(connection, True)


def tunnel(connection, mode, target, contexts):
    _, authority, _, _ = parts(target)
    first, fields = wire.request(connection)
    assert first == b"CONNECT " + authority + b" HTTP/1.1", first
    assert fields[b"host"] == authority, fields
    proxy_credentials(fields[b"proxy-authorization"], PROXY)
    assert b"authorization" not in fields and b"cookie" not in fields, fields
    if mode == "proxy-deadline":
        wire.closed(connection)
        return
    if mode == "reject":
        connection.sendall(
            b'HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm="proxy"\r\nTransfer-Encoding: chunked\r\n\r\n100000\r\n'
        )
        wire.closed(connection)
        return
    connection.sendall(
        b"HTTP/1.1 103 Early Hints\r\nLink: </proxy-only>\r\n\r\nHTTP/1.1 299 Tunnel\r\nContent-Length: 999\r\n\r\n"
        + (SETTINGS if mode == "h2-clear" else b"")
    )
    if mode == "target-untrusted":
        tls_peer.untrusted(connection, contexts["http/1.1"])
        return
    if parts(target)[0].scheme == "https":
        protocol = "h2" if mode in ("h2", "h2-proxy") else "http/1.1"
        connection = tls_peer.TlsLayer(connection, contexts[protocol])
        assert connection.ssl.selected_alpn_protocol() == protocol
    if mode == "h2-upgrade":
        fields = request(connection, target)
        assert fields[b"upgrade"].lower() == b"h2c", fields
        assert b"http2-settings" in fields, fields
        wire.upgrade(connection)
        wire.closed(connection, True)
    elif mode in ("h2", "h2-proxy", "h2-clear"):
        h2(connection, mode == "h2-clear")
    else:
        version = b"HTTP/1.0" if mode == "http10" else b"HTTP/1.1"
        request(connection, target, version=version)
        response(connection, version=version)
        wire.closed(connection)


def digest(listener, target):
    for attempt in range(2):
        with listener.accept()[0] as connection:
            connection.settimeout(180)
            fields = request(connection, target, forward=True, digest=True)
            if attempt == 0:
                assert b"authorization" not in fields
                connection.sendall(
                    b"HTTP/1.1 401 Challenge\r\nConnection: close\r\nContent-Length: 0\r\nWWW-Authenticate: "
                    + oracle.challenge("SHA-256", "auth")
                    + b"\r\n\r\n"
                )
            else:
                proof = oracle.verify(
                    fields[b"authorization"], b"GET", parts(target)[3], b"", "SHA-256", "auth"
                )
                response(
                    connection,
                    extra=b"Authentication-Info: " + oracle.parameters(proof(b"ok")) + b"\r\n",
                )
            wire.closed(connection)


def routing(listeners, mode, target):
    plan = [(listeners[0], True, PROXY, b"ok")]
    if mode == "override":
        plan.append((listeners[2], False, b"", b"ok"))
    elif mode == "identity":
        plan.append((listeners[0], True, OTHER_PROXY, b"ok"))
    elif mode == "routes":
        plan = [
            (listeners[0], True, PROXY, b"one"),
            (listeners[1], True, OTHER_PROXY, b"two"),
            (listeners[0], True, PROXY, b"one"),
        ]
    for listener, forward, credential, body in plan:
        with listener.accept()[0] as connection:
            connection.settimeout(180)
            for _ in range(2 if mode == "reuse" else 1):
                request(connection, target, forward=forward, proxy=credential)
                response(connection, body)
            wire.closed(connection)


def follow_up(listeners, mode, target):
    redirected = f"http://other.invalid:{parts(target)[0].port}/redirected?q=1"
    with listeners[0].accept()[0] as connection:
        connection.settimeout(180)
        request(connection, target, forward=True)
        if mode == "redirect":
            connection.sendall(
                b"HTTP/1.1 302 Found\r\nConnection: close\r\nContent-Length: 0\r\nLocation: "
                + redirected.encode()
                + b"\r\n\r\n"
            )
            wire.closed(connection)
    with listeners[1].accept()[0] as connection:
        connection.settimeout(180)
        request(
            connection,
            redirected if mode == "redirect" else target,
            forward=True,
            proxy=OTHER_PROXY,
            origin=mode != "redirect",
        )
        response(connection)
        wire.closed(connection)
