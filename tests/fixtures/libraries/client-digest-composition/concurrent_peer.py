import contextlib
import re

from http1_peer import incoming, initial, response, success, upload
from oracle import challenge, parameters, verify
from routing_peer import authenticated
from wire import closed, request


def concurrent(listener, mode):
    initial(listener, mode, "SHA-256", "auth", b"GET")
    with listener.accept()[0] as connection:
        proof = authenticated(connection, b"GET", b"/private/start")
        success(connection, proof, mode)
    with contextlib.ExitStack() as stack:
        pending = {}
        counts = set()
        for _ in range(2):
            connection = stack.enter_context(listener.accept()[0])
            connection.settimeout(60)
            line, fields = request(connection)
            target = line.split()[1]
            assert target in (b"/private/a", b"/private/b") and target not in pending, line
            assert upload(connection, fields) == b""
            count = int(re.search(rb"\bnc=([0-9a-f]{8})\b", fields[b"authorization"])[1], 16)
            counts.add(count)
            proof = verify(
                fields[b"authorization"], b"GET", target, b"", "SHA-256", "auth", count=count
            )
            pending[target] = connection, proof
        assert counts == {2, 3}, counts
        first, proof = pending[b"/private/a"]
        success(first, proof, mode, b"next")
        with listener.accept()[0] as connection:
            proof = authenticated(connection, b"GET", b"/private/ack", b"next")
            success(connection, proof, mode)
        second, proof = pending[b"/private/b"]
        if mode == "late-401":
            response(
                second, 401, b"", b"WWW-Authenticate: " + challenge("SHA-256", "auth") + b"\r\n"
            )
            closed(second)
            with listener.accept()[0] as connection:
                proof = authenticated(connection, b"GET", b"/private/b", b"next", 2)
                success(connection, proof, mode)
        else:
            success(second, proof, mode, b"obsolete")
    with listener.accept()[0] as connection:
        proof = authenticated(
            connection, b"GET", b"/private/final", b"next", 3 if mode == "late-401" else 2
        )
        success(connection, proof, mode)


def clear_active(listener):
    initial(listener, "clear-active", "SHA-256", "auth-int", b"GET")
    with listener.accept()[0] as held:
        fields = incoming(held, b"GET", authenticated=True)
        proof = verify(
            fields[b"authorization"], b"GET", b"/private/start", b"", "SHA-256", "auth-int"
        )
        held.sendall(
            b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 4\r\nAuthentication-Info: "
            + parameters(proof(nextnonce=b"obsolete"))
            + b"\r\n\r\n"
        )
        with listener.accept()[0] as fresh:
            incoming(fresh, b"GET", b"/private/new", False)
            held.sendall(b"done")
            closed(held)
            response(
                fresh,
                401,
                b"",
                b"WWW-Authenticate: " + challenge("SHA-256", "auth", b"fresh") + b"\r\n",
            )
            closed(fresh)
    with listener.accept()[0] as connection:
        proof = authenticated(connection, b"GET", b"/private/new", b"fresh")
        success(connection, proof, "clear-active")
    with listener.accept()[0] as connection:
        proof = authenticated(connection, b"GET", b"/private/final", b"fresh", 2)
        success(connection, proof, "clear-active")
