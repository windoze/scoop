import contextlib
import os
import socket
import subprocess
import sys
import threading

from http1_peer import incoming, initial, response, success
from oracle import challenge, parameters, verify
from wire import closed


def authenticated(connection, target, qop, count, nonce=b'nonce,\xe9"x'):
    fields = incoming(connection, b"GET", target, True)
    return verify(fields[b"authorization"], b"GET", target, b"", "SHA-256", qop, nonce, count)


def serve(listener):
    initial(listener, "nextnonce", "SHA-256", "auth-int", b"GET")
    with listener.accept()[0] as held:
        proof = authenticated(held, b"/private/start", "auth-int", 1)
        held.sendall(
            b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 4\r\nAuthentication-Info: "
            + parameters(proof(nextnonce=b"next"))
            + b"\r\n\r\n"
        )
        with listener.accept()[0] as connection:
            authenticated(connection, b"/private/new", "auth-int", 2)
            response(
                connection, 401, b"", b"WWW-Authenticate: " + challenge("SHA-256", "auth") + b"\r\n"
            )
            closed(connection)
        with listener.accept()[0] as connection:
            proof = authenticated(connection, b"/private/new", "auth", 3)
            success(connection, proof, "nextnonce")
        held.sendall(b"done")
        closed(held)
    with listener.accept()[0] as connection:
        proof = authenticated(connection, b"/private/final", "auth", 1, b"next")
        success(connection, proof, "nextnonce")


def exercise(binary):
    failures = []
    with contextlib.ExitStack() as stack:
        listeners = [stack.enter_context(socket.socket()) for _ in range(2)]
        for listener in listeners:
            listener.bind(("127.0.0.1", 0))
            listener.listen(4)
            listener.settimeout(60)

        def worker(listener):
            try:
                serve(listener)
            except BaseException as error:
                failures.append(error)

        threads = [
            threading.Thread(target=worker, args=(listener,), daemon=True) for listener in listeners
        ]
        for thread in threads:
            thread.start()
        result = subprocess.run(
            [binary, *[f"http://127.0.0.1:{s.getsockname()[1]}/private/start" for s in listeners]],
            env=os.environ,
            capture_output=True,
            text=True,
            timeout=180,
        )
        for thread in threads:
            thread.join(60)
            assert not thread.is_alive(), "Digest nextnonce peer did not finish"
        assert not failures, (failures, result)
        assert (
            result.returncode == 0
            and result.stdout == "Digest nextnonce preserves current challenge parameters\n"
            and not result.stderr
        ), result


if __name__ == "__main__":
    exercise(sys.argv[1])
    print("Digest nextnonce preserves current challenge parameters")
