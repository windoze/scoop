import contextlib
import os
import socket
import struct
import subprocess
import threading

from tls_peer import TlsLayer
from wire import begin, closed, encoded, exact, h1_request, h2_request


def event_headers():
    name, value = b"content-type", b"text/event-stream"
    return b"\x88\x00" + bytes([len(name)]) + name + bytes([len(value)]) + value


def exercise_transport(binary, mode, context, timeout):
    failures = []
    with socket.socket() as listener, socket.socket() as notify:
        for endpoint in (listener, notify):
            endpoint.bind(("127.0.0.1", 0))
            endpoint.listen(4)
            endpoint.settimeout(timeout)

        def worker():
            try:
                for index in range(1 if mode in ("h2-protocol", "refused") else 2):
                    with listener.accept()[0] as raw:
                        raw.settimeout(timeout)
                        connection = TlsLayer(raw, context) if mode.startswith("tls-") else raw
                        h2 = mode not in ("tls-eof", "refused")
                        if h2:
                            begin(connection)
                            h2_request(connection, 1)
                        else:
                            headers = h1_request(connection, b"GET / HTTP/1.1")
                            assert headers.get(b"last-event-id") == (None if index == 0 else b"one")
                        if index == 1:
                            if h2:
                                connection.sendall(encoded(1, 5, 1, b"\x89"))
                            else:
                                connection.sendall(b"HTTP/1.1 204 No Content\r\n\r\n")
                            closed(connection, h2)
                            continue
                        body = b"id: one\ndata: body\n\n"
                        if h2:
                            connection.sendall(
                                encoded(1, 4, 1, event_headers()) + encoded(0, 0, 1, body)
                            )
                        elif mode == "refused":
                            connection.sendall(
                                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\nContent-Length: "
                                + str(len(body)).encode()
                                + b"\r\n\r\n"
                                + body
                            )
                        else:
                            connection.sendall(
                                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 100\r\n\r\n"
                                + body
                            )
                        with notify.accept()[0] as signal:
                            assert exact(signal, 2) == b"go"
                            if mode == "refused":
                                listener.close()
                            signal.sendall(b"x")
                        if mode == "refused":
                            continue
                        if mode == "h2-protocol":
                            connection.sendall(encoded(3, 0, 1, struct.pack("!I", 1)))
                            closed(connection, True)
                        else:
                            raw.shutdown(socket.SHUT_WR)
                            with contextlib.suppress(ConnectionResetError):
                                while raw.recv(4096):
                                    pass
            except BaseException as error:
                failures.append(error)

        thread = threading.Thread(target=worker, daemon=True)
        thread.start()
        scheme = "https" if mode.startswith("tls-") else "http"
        host = "localhost" if mode == "refused" else "127.0.0.1"
        result = subprocess.run(
            [
                binary,
                mode,
                f"{scheme}://{host}:{listener.getsockname()[1]}/",
                str(notify.getsockname()[1]),
            ],
            capture_output=True,
            text=True,
            timeout=timeout,
            env=os.environ,
        )
        thread.join(timeout)
        assert not thread.is_alive(), (
            mode,
            "SSE transport peer did not finish",
            result.stdout,
            result.stderr,
        )
        assert not failures, (mode, failures, result.stdout, result.stderr)
        assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
        assert result.stdout == f"client SSE {mode}\n" and not result.stderr
