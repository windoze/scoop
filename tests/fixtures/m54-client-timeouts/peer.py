import gzip
import os
import socket
import struct
import subprocess
import sys
import threading

from wire import closed, exact, frame, h2_response, request, response, send_frame


def serve_http1(connection, mode):
    if mode == "pool":
        request(connection, b"GET /held HTTP/1.1")
        response(connection, b"held")
        request(connection, b"GET /after HTTP/1.1")
        response(connection, b"ok")
    else:
        fields = request(connection, b"POST / HTTP/1.1")
        assert fields[b"content-length"] == b"4"
        if mode == "decoded":
            assert exact(connection, 4) == b"send"
            compressed = gzip.compress(b"unfinished representation")
            connection.sendall(
                b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: "
                + str(len(compressed)).encode()
                + b"\r\n\r\n"
                + compressed[:10]
            )
        else:
            assert fields[b"expect"] == b"100-continue"
            reply = b"HTTP/1.1 103 Early Hints\r\n\r\n"
            if mode in ("boundary", "total-boundary"):
                reply += b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
            connection.sendall(reply)
    closed(connection)


def serve_http2(connection):
    assert exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    send_frame(connection, 4, 0, 0, struct.pack("!HI", 3, 2))
    headers = set()
    ended = set()
    reset = False
    replied = False
    while not (5 in ended and reset):
        kind, flags, stream, payload = frame(connection)
        if kind == 1:
            assert stream in (1, 3, 5) and stream not in headers and flags & 4
            headers.add(stream)
            if flags & 1:
                ended.add(stream)
        elif kind == 0:
            assert stream in headers and not payload
            if flags & 1:
                ended.add(stream)
        elif kind == 3:
            assert stream == 1 and payload == struct.pack("!I", 8)
            assert not reset
            reset = True
        elif kind == 4:
            assert stream == 0
            if not flags & 1:
                send_frame(connection, 4, 1, 0)
        else:
            assert kind == 8
        if 3 in ended and not replied:
            assert 1 in headers
            h2_response(connection, 3)
            replied = True
    assert headers == {1, 3, 5} and replied
    h2_response(connection, 5)
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
                        serve_http2(connection)
                    else:
                        serve_http1(connection, mode)
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
            assert not thread.is_alive(), "timeout peer did not finish"
        if failures:
            raise failures[0]
        assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
        assert result.stdout == f"timeout {mode}\n" and not result.stderr


for mode in ("pool", "headers", "boundary", "total-boundary", "decoded"):
    exercise(sys.argv[1], mode)
print("independent HTTP1 deadlines passed")
exercise(sys.argv[1], "h2")
print("independent HTTP2 timeout isolation passed")
