import gzip

import auth_wire as http
import oracle
import wire

ALGORITHM = "SHA-256-sess"
NONCE = b'nonce,\xe9"x'


def digest(listener, mode, target, contexts):
    algorithm = "SHA-512-256-sess" if mode == "digest-connect" else ALGORITHM
    connect = http.parts(target)[0].scheme == "https"
    nonce = NONCE
    attempts = 1 if mode == "digest-nohash" else 3 if mode == "digest-stale" else 2
    for attempt in range(attempts):
        with http.connect(listener, contexts) as connection:
            method = b"CONNECT" if connect else b"POST"
            fields = http.request(connection, target, method)
            assert b"authorization" not in fields, fields
            if not attempt:
                http.credentials(fields, None)
                extra = (
                    {"domain": "[bad] http://other.invalid/private"}
                    if mode == "digest-domain"
                    else {}
                )
                http.challenge(connection, oracle.challenge(algorithm, "auth-int", **extra))
                continue
            if mode == "digest-stale" and attempt == 1:
                oracle.verify(
                    fields[b"proxy-authorization"],
                    method,
                    http.parts(target)[3],
                    b"payload",
                    algorithm,
                    "auth-int",
                )
                nonce = b"changed nonce"
                http.challenge(
                    connection, oracle.challenge(algorithm, "auth-int", nonce=nonce, stale="true")
                )
                continue
            if connect:
                proof = oracle.verify(
                    fields[b"proxy-authorization"],
                    b"CONNECT",
                    http.parts(target)[1],
                    b"",
                    algorithm,
                    "auth-int",
                    nonce=nonce,
                )
                info = proof(b"")
                if mode == "digest-bad-connect":
                    info["rspauth"] = b"0" * 64
                    connection.sendall(
                        b"HTTP/1.1 299 Tunnel\r\nProxy-Authentication-Info: "
                        + oracle.parameters(info)
                        + b"\r\n\r\n"
                    )
                    wire.closed(connection)
                    continue
                connection = http.tunnel(
                    connection,
                    target,
                    contexts,
                    b"Proxy-Authentication-Info: " + oracle.parameters(info) + b"\r\n",
                    h2=mode == "digest-h2-connect",
                )
                if mode == "digest-h2-connect":
                    http.http2(connection)
                    continue
                fields = http.request(connection, target, forward=False)
                assert b"authorization" not in fields
                assert http.entity(connection, fields) == b"payload"
                http.response(connection)
            else:
                body = http.entity(connection, fields)
                if mode == "digest-gzip":
                    assert (
                        fields[b"content-encoding"] == b"gzip"
                        and gzip.decompress(body) == b"payload"
                    )
                else:
                    assert body == b"payload", body
                proof = oracle.verify(
                    fields[b"proxy-authorization"],
                    b"POST",
                    http.parts(target)[3],
                    body,
                    algorithm,
                    "auth-int",
                    nonce=nonce,
                )
                if mode == "digest-bad-trailer":
                    authenticated_response(
                        connection, {b"Proxy-Authentication-Info": proof}, bad=True
                    )
                else:
                    http.response(
                        connection,
                        extra=b"Proxy-Authentication-Info: " + oracle.parameters(proof()) + b"\r\n",
                    )
            wire.closed(connection)
    if mode in ("digest-domain", "digest-clear"):
        next_target = (
            f"http://other.invalid:{http.parts(target)[0].port}/outside?q=2"
            if mode == "digest-domain"
            else target.split("/a%2F", 1)[0] + "/second?q=2"
        )
        for attempt in range(2 if mode == "digest-clear" else 1):
            with http.connect(listener, contexts) as connection:
                fields = http.request(connection, next_target, b"GET")
                if mode == "digest-clear" and attempt == 0:
                    http.credentials(fields, None)
                    http.challenge(connection, oracle.challenge(algorithm, "auth-int"))
                    continue
                proof = oracle.verify(
                    fields[b"proxy-authorization"],
                    b"GET",
                    http.parts(next_target)[3],
                    b"",
                    algorithm,
                    "auth-int",
                    count=1 if mode == "digest-clear" else 2,
                )
                http.response(
                    connection,
                    extra=b"Proxy-Authentication-Info: " + oracle.parameters(proof()) + b"\r\n",
                )
                wire.closed(connection)


def authenticated_response(connection, proofs, *, bad=False):
    body = gzip.compress(b"done", mtime=0)
    headers = (
        b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Encoding: gzip\r\nTransfer-Encoding: chunked\r\nTrailer: "
        + b", ".join(proofs)
        + b"\r\n"
    )
    trailers = b""
    for name, (proof, next_nonce) in (
        (name, value if isinstance(value, tuple) else (value, None))
        for name, value in proofs.items()
    ):
        info = proof(body)
        rspauth = info.pop("rspauth")
        tail = {"rspauth": b"0" * 64 if bad else rspauth}
        if next_nonce is not None:
            tail["nextnonce"] = next_nonce
        headers += name + b": " + oracle.parameters(info) + b"\r\n"
        trailers += name + b": " + oracle.parameters(tail) + b"\r\n"
    connection.sendall(
        headers + b"\r\n" + f"{len(body):x}\r\n".encode() + body + b"\r\n0\r\n" + trailers + b"\r\n"
    )


def double(listener, proxy, contexts):
    target = proxy + "/resource?q=%2F"
    for attempt in range(4):
        if attempt == 3:
            target = proxy + "/second?q=2"
        with http.connect(listener, contexts) as connection:
            fields = http.request(connection, target, b"GET")
            assert http.entity(connection, fields) == b""
            if attempt == 0:
                http.credentials(fields, None)
                http.credentials(fields, None, b"authorization")
                http.challenge(
                    connection, oracle.challenge(ALGORITHM, "auth-int", nonce=b"proxy nonce")
                )
                continue
            proxy_proof = oracle.verify(
                fields[b"proxy-authorization"],
                b"GET",
                http.parts(target)[3],
                b"",
                ALGORITHM,
                "auth-int",
                nonce=b"proxy next" if attempt == 3 else b"proxy nonce",
                count=1 if attempt in (1, 3) else 2,
            )
            if attempt == 1:
                http.credentials(fields, None, b"authorization")
                http.challenge(
                    connection,
                    oracle.challenge(ALGORITHM, "auth-int", nonce=b"origin nonce"),
                    origin=True,
                )
                continue
            origin_proof = oracle.verify(
                fields[b"authorization"],
                b"GET",
                http.parts(target)[3],
                b"",
                ALGORITHM,
                "auth-int",
                nonce=b"origin next" if attempt == 3 else b"origin nonce",
            )
            authenticated_response(
                connection,
                {
                    b"Proxy-Authentication-Info": (proxy_proof, b"proxy next"),
                    b"Authentication-Info": (origin_proof, b"origin next"),
                },
            )
            wire.closed(connection)
