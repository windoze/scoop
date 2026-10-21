import select
import socket
import struct
import subprocess
import sys
import threading
import time

from wire import begin, closed, encoded, frame, request


def reset(connection, expected, ping=False):
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        if ping:
            connection.sendall(encoded(6, 0, 0, b"12345678"))
        if not select.select([connection], [], [], 0.08)[0]:
            continue
        kind, flags, stream, payload = frame(connection)
        if kind == 3:
            assert stream == expected and payload == struct.pack("!I", 8)
            return
        assert kind in (4, 6, 8), (kind, flags, stream, payload)
    raise AssertionError("stream did not send CANCEL")


def serve(connection, mode):
    connection.settimeout(15)
    connection.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    begin(connection, mode == "timeout")
    request(connection, 1, end=mode != "timeout")
    if mode == "normal":
        connection.sendall(encoded(1, 5, 1, b"\x89"))
        request(connection, 3, end=False)
        reset(connection, 3)
    else:
        connection.sendall(encoded(1, 5 if mode == "timeout" else 4, 1, b"\x88"))
        request(connection, 3)
        if mode == "reset":
            connection.sendall(encoded(3, 0, 1, struct.pack("!I", 8)))
        connection.sendall(encoded(1, 5, 3, b"\x89"))
        if mode == "timeout":
            reset(connection, 1, ping=True)
    request(connection, 5)
    connection.sendall(encoded(1, 5, 5, b"\x89"))
    closed(connection)


def exercise(binary, mode):
    failures = []
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(4)
        listener.settimeout(25)

        def worker():
            try:
                with listener.accept()[0] as connection:
                    serve(connection, mode)
            except BaseException as error:
                failures.append(error)

        workers = [threading.Thread(target=worker, daemon=True) for _ in range(2)]
        for thread in workers:
            thread.start()
        result = subprocess.run(
            [binary, mode, str(listener.getsockname()[1])],
            capture_output=True,
            text=True,
            timeout=60,
        )
        for thread in workers:
            thread.join(25)
            assert not thread.is_alive(), (mode, "peer did not finish")
        assert result.returncode == 0, (
            mode,
            result.returncode,
            result.stdout,
            result.stderr,
            failures,
        )
        assert not failures, (mode, failures)
        assert result.stdout == f"{mode} notices passed\n" and not result.stderr


for mode in ("timeout", "reset", "normal"):
    exercise(sys.argv[1], mode)
print("independent HTTP/2 stream failure notices passed")
