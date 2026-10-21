import os
import socket
import struct
import subprocess
import sys
import threading

from wire import (
    await_reset,
    begin_http2,
    closed,
    frame_bytes,
    h1_response,
    next_request,
    request,
    send_frame,
    settings,
)


def serve(connection, mode):
    connection.settimeout(180)
    method, target, fields, body = request(connection)
    assert target == (b"/first" if mode == "cancel" else b"/")
    assert method == (b"HEAD" if mode == "head" else b"GET" if mode == "cancel" else b"POST")
    assert body == (b"" if mode in ("head", "cancel") else "请求一次".encode())
    offered = settings(fields)
    if mode in ("declined", "required", "malformed"):
        if mode == "malformed":
            connection.sendall(
                b"HTTP/1.1 101 Wrong protocol\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n"
            )
        else:
            h1_response(connection, b"declined")
        if mode == "declined":
            method, target, fields, body = request(connection)
            assert method == b"GET" and target == b"/next" and not body
            assert b"upgrade" not in fields and b"http2-settings" not in fields
            h1_response(connection, b"next")
        closed(connection)
        return
    wire = b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: h2c\r\n\r\n"
    wire += frame_bytes(4, 0, 0, struct.pack("!HI", 3, 2))
    if mode == "head":
        wire += frame_bytes(1, 5, 1, b"\x88\x0f\x0d\x03123")
    else:
        wire += frame_bytes(1, 4, 1, b"\x88")
        wire += frame_bytes(0, 0, 1, b"pending" if mode == "cancel" else b"post-result")
        if mode == "post":
            wire += frame_bytes(1, 5, 1, b"\x00\x05x-end\x03yes")
    # The H1 response and the H2 suffix are deliberately sent together.
    connection.sendall(wire)
    begin_http2(connection, offered)
    pending = set()
    assert next_request(connection, pending) == 3
    send_frame(connection, 1, 4, 3, b"\x88")
    send_frame(connection, 0, 1, 3, b"other" if mode == "cancel" else b"next")
    if mode == "cancel":
        await_reset(connection, pending)
        assert next_request(connection, pending) == 5
        send_frame(connection, 1, 5, 5, b"\x89")
    closed(connection, h2=True, pending=pending)


def exercise(binary, mode):
    failures = []
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(4)
        listener.settimeout(180)

        def worker():
            try:
                with listener.accept()[0] as connection:
                    serve(connection, mode)
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
            env=os.environ,
        )
        for thread in workers:
            thread.join(180)
            assert not thread.is_alive(), "h2c peer did not finish"
        if failures:
            raise failures[0]
        assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
        assert result.stdout == f"h2c {mode}\n" and not result.stderr


for mode in ("post", "head"):
    exercise(sys.argv[1], mode)
print("independent h2c POST and HEAD passed")
for mode in ("declined", "required", "malformed"):
    exercise(sys.argv[1], mode)
print("independent h2c refusal passed")
exercise(sys.argv[1], "cancel")
print("independent h2c stream cancellation passed")
