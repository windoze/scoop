import contextlib
import hashlib
import os
import socket
import subprocess
import sys
import threading
import urllib.request

from http1_peer import incoming, response
from oracle import parameters, quoted
from wire import closed


def configuration(mode, second=False):
    return {
        "algorithm": "MD5" if mode == "algorithm" and not second else "SHA-256",
        "realm": "second" if mode == "realm" and second else "private",
        "nonce": "next" if mode == "nonce" and second else "nonce",
        "opaque": "second" if mode == "opaque" and second else "first",
        "qop": "auth-int" if mode == "qop" and second else "auth",
        **({"charset": "UTF-8"} if mode == "charset" and second else {}),
    }


def challenge(config):
    values = dict(config)
    qop = values.pop("qop")
    return b"Digest " + parameters(values) + b", qop=" + quoted(qop)


def verify(header, config, mode, count):
    scheme, raw = header.decode("latin-1").split(" ", 1)
    assert scheme == "digest", header
    values = urllib.request.parse_keqv_list(urllib.request.parse_http_list(raw))
    for name in ("algorithm", "realm", "nonce", "opaque", "qop"):
        assert values[name] == config[name], (name, values, config)
    assert values["nc"] == f"{count:08x}" and values["uri"] == "/private/start", values
    encoding = "utf-8" if "charset" in config else "latin-1"
    username, password = ("café", "päss") if mode == "charset" else ("Mufasa", "Circle of Life")

    def digest(data):
        return hashlib.new(config["algorithm"].lower().replace("-", ""), data).hexdigest()

    ha1 = digest(f"{username}:{config['realm']}:{password}".encode(encoding))
    a2 = "GET:/private/start" + (":" + digest(b"") if config["qop"] == "auth-int" else "")
    expected = digest(
        f"{ha1}:{config['nonce']}:{values['nc']}:{values['cnonce']}:{config['qop']}:{digest(a2.encode())}".encode()
    )
    assert values["response"] == expected, values


def serve(listener, mode):
    separate = mode in ("qop", "nonce", "opaque")
    first, second = configuration(mode), configuration(mode, True)
    with listener.accept()[0] as connection:
        incoming(connection, b"GET")
        response(connection, 401, b"", b"WWW-Authenticate: " + challenge(first) + b"\r\n")
        closed(connection)
    with listener.accept()[0] as connection:
        fields = incoming(connection, b"GET", authenticated=True)
        verify(fields[b"authorization"], first, mode, 1)
        if separate:
            response(connection)
        else:
            response(connection, 401, b"", b"WWW-Authenticate: " + challenge(second) + b"\r\n")
        closed(connection)
    if separate:
        with listener.accept()[0] as connection:
            incoming(connection, b"GET")
            response(connection, 401, b"", b"WWW-Authenticate: " + challenge(second) + b"\r\n")
            closed(connection)
    with listener.accept()[0] as connection:
        fields = incoming(connection, b"GET", authenticated=True)
        verify(fields[b"authorization"], second, mode, 2 if mode in ("qop", "opaque") else 1)
        response(connection)
        closed(connection)


def exercise(binary, mode):
    failures = []
    with contextlib.ExitStack() as stack:
        listeners = [stack.enter_context(socket.socket()) for _ in range(2)]
        for listener in listeners:
            listener.bind(("127.0.0.1", 0))
            listener.listen(2)
            listener.settimeout(60)

        def worker(listener):
            try:
                serve(listener, mode)
            except BaseException as error:
                failures.append(error)

        threads = [
            threading.Thread(target=worker, args=(listener,), daemon=True) for listener in listeners
        ]
        for thread in threads:
            thread.start()
        result = subprocess.run(
            [
                binary,
                mode,
                *[f"http://127.0.0.1:{s.getsockname()[1]}/private/start" for s in listeners],
            ],
            env=dict(os.environ, SCOOP_GC_STRESS_MOVE="1"),
            capture_output=True,
            text=True,
            timeout=180,
        )
        for thread in threads:
            thread.join(60)
            assert not thread.is_alive(), mode
        assert not failures, (mode, failures, result)
        assert (
            result.returncode == 0
            and result.stdout == f"Digest transition {mode}\n"
            and not result.stderr
        ), result


if __name__ == "__main__":
    for mode in ("qop", "nonce", "opaque", "algorithm", "charset", "realm"):
        exercise(sys.argv[1], mode)
    print("Digest challenge changes preserve nonce counts and select current parameters")
