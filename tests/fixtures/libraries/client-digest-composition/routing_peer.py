import socket

from http1_peer import incoming, initial, response, success, upload
from oracle import challenge, parameters, verify
from wire import closed


def challenged(listener, method, domain=False):
    value = challenge(
        "SHA-256", "auth", **({"domain": b"/private/ http://ignored.invalid/"} if domain else {})
    )
    with listener.accept()[0] as connection:
        incoming(connection, method)
        response(connection, 401, b"", b"WWW-Authenticate: " + value + b"\r\n")
        closed(connection)


def authenticated(connection, method, target, nonce=b'nonce,\xe9"x', count=1):
    fields = incoming(connection, method, target, True)
    assert upload(connection, fields) == b""
    return verify(fields[b"authorization"], method, target, b"", "SHA-256", "auth", nonce, count)


def routing(listener, mode):
    with socket.socket() as destination:
        destination.bind(("127.0.0.1", 0))
        destination.listen(1)
        destination.settimeout(60)
        challenged(listener, b"POST", mode == "domain")
        with listener.accept()[0] as connection:
            proof = authenticated(connection, b"POST", b"/private/start")
            response(
                connection,
                303,
                b"",
                b"Location: /private/sibling?q=x\r\nAuthentication-Info: "
                + parameters(proof(b"", nextnonce=b"next"))
                + b"\r\n",
            )
            closed(connection)
        with listener.accept()[0] as connection:
            authenticated(connection, b"GET", b"/private/sibling?q=x", b"next")
            response(connection, 307, b"", b"Location: /outside\r\n")
            closed(connection)
        with listener.accept()[0] as connection:
            if mode == "domain":
                incoming(connection, b"GET", b"/outside", False)
            else:
                authenticated(connection, b"GET", b"/outside", b"next", 2)
            response(
                connection,
                302,
                b"",
                f"Location: http://127.0.0.1:{destination.getsockname()[1]}/target\r\n".encode(),
            )
            closed(connection)
        with destination.accept()[0] as connection:
            incoming(connection, b"GET", b"/target", False)
            response(connection)
            closed(connection)


def retry(listener):
    challenged(listener, b"GET")
    with listener.accept()[0] as connection:
        proof = authenticated(connection, b"GET", b"/private/start")
        response(
            connection,
            503,
            b"",
            b"Authentication-Info: " + parameters(proof(b"", nextnonce=b"next")) + b"\r\n",
        )
        closed(connection)
    with listener.accept()[0] as connection:
        proof = authenticated(connection, b"GET", b"/private/start", b"next")
        success(connection, proof, "retry")


def no_preempt(listener):
    for count in (1, 2):
        initial(listener, "no-preempt", "SHA-256", "auth", b"GET")
        with listener.accept()[0] as connection:
            proof = authenticated(connection, b"GET", b"/private/start", count=count)
            success(connection, proof, "no-preempt")
