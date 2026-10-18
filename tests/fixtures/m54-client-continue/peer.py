import os
import socket
import struct
import subprocess
import sys
import threading

from wire import (
    closed,
    control,
    exact,
    frame,
    frame_bytes,
    h1_response,
    h2_body,
    h2_request,
    h2_response,
    request_head,
    send_frame,
)


def serve_h1(connection, mode):
    line, fields = request_head(connection)
    if mode == "empty":
        assert line == b"GET / HTTP/1.1" and b"expect" not in fields
        assert fields[b"content-length"] == b"0"
        h1_response(connection)
        closed(connection)
        return
    assert line == b"POST / HTTP/1.1" and fields[b"expect"].lower() == b"100-continue"
    if mode in ("continue", "cancel"):
        connection.sendall(b"HTTP/1.1 103 Early Hints\r\n\r\n")
    if mode == "cancel":
        closed(connection)
        return
    if mode == "reject":
        h1_response(connection, 417, b"no")
        closed(connection)
        return
    if mode != "timeout":
        connection.sendall(b"HTTP/1.1 100 Continue\r\n\r\n")
    body = exact(connection, int(fields[b"content-length"]))
    assert body == "一次上传".encode()
    if mode in ("continue", "timeout"):
        connection.sendall(b"HTTP/1.1 100 Continue\r\n\r\n")
    if mode == "h2c":
        assert fields[b"upgrade"] == b"h2c"
        connection.sendall(
            b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: h2c\r\n\r\n"
            + frame_bytes(4, 0, 0)
        )
        assert exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
        while True:
            kind, flags, stream, payload = frame(connection)
            control(connection, kind, flags, stream, payload)
            if kind == 4 and not flags & 1:
                break
        h2_response(connection, 1)
        closed(connection, h2=True)
    else:
        h1_response(connection)
        closed(connection)


def serve_h2(connection):
    assert exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    send_frame(connection, 4, 0, 0, struct.pack("!HI", 3, 2))
    assert h2_request(connection) == 1
    assert h2_request(connection) == 3
    send_frame(connection, 1, 4, 3, b"\x08\x03100")
    assert h2_body(connection, 3) == b"second"
    h2_response(connection, 3)
    send_frame(connection, 1, 4, 1, b"\x08\x03100")
    assert h2_body(connection, 1) == b"first"
    h2_response(connection, 1)
    closed(connection, h2=True)


def exercise(binary, mode):
    failures = []
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(4)
        listener.settimeout(180)

        def worker():
            try:
                with listener.accept()[0] as connection:
                    connection.settimeout(180)
                    if mode == "h2":
                        serve_h2(connection)
                    else:
                        serve_h1(connection, mode)
            except BaseException as error:
                failures.append(error)

        workers = [threading.Thread(target=worker, daemon=True) for _ in range(2)]
        for thread in workers:
            thread.start()
        address = f"http://127.0.0.1:{listener.getsockname()[1]}/"
        result = subprocess.run(
            [binary, mode, address],
            capture_output=True,
            text=True,
            timeout=180,
            env=dict(os.environ, SCOOP_GC_STRESS_MOVE="1"),
        )
        for thread in workers:
            thread.join(180)
            assert not thread.is_alive(), "Continue peer did not finish"
        if failures:
            raise failures[0]
        assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
        assert result.stdout == f"continue {mode}\n" and not result.stderr


for mode in ("continue", "timeout", "reject", "cancel", "empty"):
    exercise(sys.argv[1], mode)
print("independent HTTP1 Continue passed")
exercise(sys.argv[1], "h2")
print("independent HTTP2 Continue passed")
exercise(sys.argv[1], "h2c")
print("independent h2c Continue passed")
