import gzip

from oracle import challenge, parameters, verify
from wire import chunked, closed, exact, request


def response(connection, status=200, body=b"done", fields=b""):
    connection.sendall(
        f"HTTP/1.1 {status} Actual response\r\nConnection: close\r\nContent-Length: {len(body)}\r\n".encode()
        + fields
        + b"\r\n"
        + body
    )


def incoming(connection, method=b"POST", target=b"/private/start", authenticated=False):
    connection.settimeout(60)
    first, fields = request(connection)
    assert first == method + b" " + target + b" HTTP/1.1", first
    assert (b"authorization" in fields) == authenticated, fields
    return fields


def upload(connection, fields):
    if fields.get(b"expect", b"").lower() == b"100-continue":
        connection.sendall(b"HTTP/1.1 100 Continue\r\n\r\n")
    if b"transfer-encoding" in fields:
        body, trailers = chunked(connection)
        assert trailers == [(b"x-upload", b"checked")], trailers
        assert gzip.decompress(body) == b"payload"
        return body
    return exact(connection, int(fields.get(b"content-length", b"0")))


def settings(mode):
    if "_" in mode:
        algorithm, qop = mode.split("_", 1)
        return algorithm, None if qop == "legacy" else qop
    algorithm = (
        "SHA-512-256" if mode == "preference" else "MD5" if mode == "default-md5" else "SHA-256"
    )
    qop = (
        "auth-int"
        if mode
        in (
            "provided",
            "provided-sent",
            "nohash",
            "gzip",
            "trailer",
            "integrity-preference",
            "bad-body",
            "bad-trailer-duplicate",
        )
        else "auth"
    )
    return algorithm, qop


def initial(listener, mode, algorithm, qop, method=b"POST", target=b"/private/start"):
    offered = "auth, auth-int" if mode in ("integrity-preference", "auth-preference") else qop
    extra = {"charset": "uTf-8"} if mode in ("utf8", "userhash") else {}
    if mode == "userhash":
        extra["userhash"] = "true"
    value = challenge(algorithm, offered, **extra)
    if mode == "default-md5":
        value = value.replace(b", algorithm=MD5", b"")
    if mode == "preference":
        value = challenge("MD5", qop) + b", " + challenge("SHA-256", qop) + b", " + value
    replacements = {
        "invalid-token": b"Digest abc==",
        "invalid-nonce": b'Digest realm="private", algorithm=SHA-256, nonce=""',
        "invalid-realm": b'Digest algorithm=SHA-256, nonce="n"',
        "invalid-syntax": b'Digest realm="unterminated',
        "unsupported-algorithm": challenge("SHA-999", qop),
        "unsupported-qop": challenge(algorithm, "future"),
        "unsupported-charset": challenge(algorithm, qop, charset="Latin-1"),
        "unsupported-realm": challenge(algorithm, qop, realm=b"other"),
    }
    value = replacements.get(mode, value)
    with listener.accept()[0] as connection:
        fields = incoming(connection, method, target)
        if mode == "provided-sent":
            assert upload(connection, fields) == b"payload"
        response(
            connection,
            401,
            b"deny" if mode.startswith("unsupported-") else b"",
            b"WWW-Authenticate: " + value + b"\r\n",
        )
        closed(connection)


def success(connection, proof, mode, nextnonce=None):
    body = gzip.compress(b"done", mtime=0) if mode == "gzip" else b"done"
    info = proof(body)
    if nextnonce is not None:
        info["nextnonce"] = nextnonce
    changes = {
        "bad-proof": ("rspauth", b"0" * 64),
        "bad-hex": ("rspauth", b"invalid"),
        "bad-qop": ("qop", "other"),
        "bad-nc": ("nc", "00000002"),
        "bad-cnonce": ("cnonce", b"wrong"),
        "bad-nextnonce": ("nextnonce", b""),
    }
    if mode in changes:
        name, value = changes[mode]
        info[name] = value
    if mode == "bad-missing":
        del info["rspauth"]
    wire_info = b"Authentication-Info: " + parameters(info) + b"\r\n"
    if mode == "bad-syntax":
        wire_info = b'Authentication-Info: rspauth="unterminated\r\n'
    if mode in ("trailer", "gzip", "bad-trailer-duplicate"):
        extra = b"Content-Encoding: gzip\r\n" if mode == "gzip" else b""
        if mode == "bad-trailer-duplicate":
            extra += b"Authentication-Info: qop=auth-int\r\n"
        connection.sendall(
            b"HTTP/1.1 200 OK\r\nConnection: close\r\nTransfer-Encoding: chunked\r\n"
            b"Trailer: Authentication-Info\r\n"
            + extra
            + b"\r\n"
            + f"{len(body):x}\r\n".encode()
            + body
            + b"\r\n0\r\n"
            + wire_info
            + b"\r\n"
        )
    else:
        response(
            connection,
            body=b"evil" if mode == "bad-body" else body,
            fields=b"" if mode == "no-info" else wire_info,
        )
    closed(connection)


def serve(listener, mode):
    algorithm, qop = settings(mode)
    initial(listener, mode, algorithm, qop)
    if mode.startswith(("invalid-", "unsupported-")) or mode in ("nohash", "provided-sent"):
        return
    nonce = b'nonce,\xe9"x'
    with listener.accept()[0] as connection:
        fields = incoming(connection, authenticated=True)
        body = upload(connection, fields)
        if mode != "gzip":
            assert body == b"payload", body
        proof = verify(
            fields[b"authorization"], b"POST", b"/private/start", body, algorithm, qop, mode=mode
        )
        if mode in ("stale", "rejected", "no-stale", "limit"):
            selected = nonce if mode == "rejected" else b"next"
            response(
                connection,
                401,
                b"",
                b"WWW-Authenticate: "
                + challenge(
                    algorithm, qop, selected, **({} if mode == "no-stale" else {"stale": "true"})
                )
                + b"\r\n",
            )
            closed(connection)
        else:
            success(connection, proof, mode, b"next" if mode == "nextnonce" else None)
    if mode == "stale":
        with listener.accept()[0] as connection:
            fields = incoming(connection, authenticated=True)
            body = upload(connection, fields)
            proof = verify(
                fields[b"authorization"], b"POST", b"/private/start", body, algorithm, qop, b"next"
            )
            success(connection, proof, mode)
    if mode not in ("cache", "nextnonce", "preempt-rejected", "clear"):
        return
    target = b"/private/second?q=%2F"
    if mode == "clear":
        initial(listener, mode, algorithm, qop, b"GET", target)
    with listener.accept()[0] as connection:
        fields = incoming(connection, b"GET", target, True)
        assert upload(connection, fields) == b""
        proof = verify(
            fields[b"authorization"],
            b"GET",
            target,
            b"",
            algorithm,
            qop,
            b"next" if mode == "nextnonce" else nonce,
            1 if mode in ("nextnonce", "clear") else 2,
        )
        if mode == "preempt-rejected":
            response(
                connection,
                401,
                b"",
                b"WWW-Authenticate: " + challenge(algorithm, qop, b"next") + b"\r\n",
            )
            closed(connection)
        else:
            success(connection, proof, mode)
    if mode == "preempt-rejected":
        with listener.accept()[0] as connection:
            fields = incoming(connection, b"GET", target, True)
            proof = verify(fields[b"authorization"], b"GET", target, b"", algorithm, qop, b"next")
            success(connection, proof, mode)
