import base64

import auth_wire as http
import wire

BASIC = b"Basic " + base64.b64encode("José:päss:word".encode())
ORIGIN = b"Basic " + base64.b64encode(b"origin:secret")
CHALLENGE = b'Basic realm="private", charset="UtF-8"'


def basic(listeners, mode, proxy, target, contexts):
    if mode == "basic-redirect":
        redirect(listeners, target, contexts)
        return
    connect = http.parts(target)[0].scheme == "https"
    outer = mode == "basic-https-proxy"
    delivered = mode in ("basic-static", "basic-wrong-origin", "basic-wrong-field")
    attempts = (
        1
        if delivered or mode in ("basic-malformed", "basic-sent")
        else 3
        if mode == "basic-layered"
        else 2
    )
    for attempt in range(attempts):
        with http.connect(listeners[0], contexts, outer) as connection:
            fields = http.request(connection, target, b"CONNECT" if connect else b"POST")
            http.credentials(
                fields, b"Bearer static" if mode == "basic-static" else BASIC if attempt else None
            )
            http.credentials(fields, ORIGIN if attempt == 2 else None, b"authorization")
            if mode == "basic-sent":
                assert http.entity(connection, fields) == b"payload"
            if mode == "basic-wrong-field":
                http.response(connection, b"deny", b"WWW-Authenticate: " + CHALLENGE + b"\r\n", 407)
                wire.closed(connection)
            elif attempt == 0 or mode in ("basic-rejected", "basic-limit"):
                value = (
                    b'Basic realm="unterminated'
                    if mode == "basic-malformed"
                    else b'Basic realm="changed"'
                    if attempt and mode == "basic-limit"
                    else CHALLENGE
                )
                http.challenge(connection, value, delivered=delivered)
            elif mode == "basic-layered" and attempt == 1:
                http.challenge(connection, b'Basic realm="origin"', origin=True)
            else:
                if connect:
                    connection = http.tunnel(connection, target, contexts)
                    fields = http.request(connection, target, forward=False)
                    http.credentials(fields, None, b"authorization")
                assert http.entity(connection, fields) == b"payload"
                http.response(connection)
                wire.closed(connection)


def bearer(listeners, mode, target, contexts):
    connect = mode == "bearer-refresh-connect"
    for attempt in range(2):
        with http.connect(listeners[0], contexts) as connection:
            fields = http.request(connection, target, b"CONNECT" if connect else b"POST")
            expected = b"Bearer fresh" if attempt else b"Bearer old" if connect else None
            http.credentials(fields, expected)
            http.credentials(fields, None, b"authorization")
            if attempt == 0:
                http.challenge(connection, b'Bearer realm="private", error="invalid_token"')
            else:
                if connect:
                    connection = http.tunnel(connection, target, contexts)
                    fields = http.request(connection, target, forward=False)
                assert http.entity(connection, fields) == b"payload"
                http.response(connection)
                wire.closed(connection)


def nested(listeners, mode, token, target, contexts):
    connect = mode == "nested-bearer-connect"
    for attempt in range(2):
        if attempt:
            with http.connect(listeners[1], contexts) as token_connection:
                fields = http.request(token_connection, token, b"GET", forward=False)
                http.credentials(fields, None, b"authorization")
                assert http.entity(token_connection, fields) == b""
                http.response(token_connection)
                wire.closed(token_connection)
        with http.connect(listeners[0], contexts) as connection:
            fields = http.request(connection, target, b"CONNECT" if connect else b"GET")
            expected = (
                b"Bearer nested" if connect else b"Basic " + base64.b64encode(b"nested:password")
            )
            http.credentials(fields, expected if attempt else None)
            http.credentials(fields, None, b"authorization")
            if attempt == 0:
                http.challenge(connection, b"Bearer" if connect else CHALLENGE)
            else:
                if connect:
                    connection = http.tunnel(connection, target, contexts)
                    fields = http.request(connection, target, b"GET", forward=False)
                assert http.entity(connection, fields) == b""
                http.response(connection)
                wire.closed(connection)


def isolation(listener, proxy, contexts):
    expected = [
        (None, b"origin-1"),
        (b"proxy-2", b"origin-1"),
        (b"proxy-3", b"origin-1"),
        (b"proxy-2", b"origin-1"),
        (b"proxy-4", b"origin-5"),
        (b"proxy-6", b"origin-5"),
        (None, b"origin-5"),
    ]
    for token, origin in expected:
        with http.connect(listener, contexts) as connection:
            fields = http.request(
                connection, proxy + "/resource", b"GET", forward=token is not None
            )
            http.credentials(fields, None if token is None else b"Bearer " + token)
            http.credentials(fields, b"Bearer " + origin, b"authorization")
            assert http.entity(connection, fields) == b""
            http.response(connection)
            wire.closed(connection)


def redirect(listeners, target, contexts):
    changed = f"http://other.invalid:{http.parts(target)[0].port}/changed"
    for index, url in enumerate((target, changed)):
        token = base64.b64encode(b"first:password" if index == 0 else b"next:password")
        for attempt in range(2):
            with http.connect(listeners[index], contexts) as connection:
                fields = http.request(connection, url, b"GET")
                http.credentials(fields, b"Basic " + token if attempt else None)
                http.credentials(
                    fields, b"Bearer original" if index == 0 else None, b"authorization"
                )
                if not attempt:
                    http.challenge(connection, CHALLENGE)
                else:
                    http.response(
                        connection,
                        extra=b"Location: " + changed.encode() + b"\r\n" if index == 0 else b"",
                        status=302 if index == 0 else 200,
                    )
                    wire.closed(connection)
