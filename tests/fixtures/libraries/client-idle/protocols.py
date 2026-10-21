import base64
import gzip
import select
import socket
import struct
import time

from wire import begin, closed, encoded, exact, frame, h1_request, h2_request, h2_requests


def reset_with_traffic(connection, other_stream=False):
    deadline = time.monotonic() + 30
    pending_ping = False
    while time.monotonic() < deadline:
        if not pending_ping:
            connection.sendall(encoded(6, 0, 0, b"12345678"))
            if other_stream:
                connection.sendall(encoded(0, 0, 3, b"x"))
            pending_ping = True
        ready = select.select([connection], [], [], 0.2)[0]
        while ready:
            kind, flags, stream, payload = frame(connection)
            if kind == 3:
                assert stream == 1 and payload == struct.pack("!I", 8), (stream, payload)
                return
            if kind == 6:
                assert flags == 1 and stream == 0 and payload == b"12345678"
                pending_ping = False
            else:
                assert kind in (4, 8), (kind, flags, stream, payload)
            ready = select.select([connection], [], [], 0)[0]
        time.sleep(0.05)
    raise AssertionError("client did not cancel its stalled stream")


def serve_http1(connection, mode):
    if mode == "h1-progress":
        h1_request(connection, b"GET / HTTP/1.1")
        wire = b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nx-pad: " + b"p" * 144 + b"\r\n\r\n"
        for offset in range(0, len(wire), 12):
            connection.sendall(wire[offset : offset + 12])
            time.sleep(0.35)
        connection.sendall(b"abc")
    else:
        fields = h1_request(connection, b"POST / HTTP/1.1")
        assert fields[b"content-length"] == b"4" and exact(connection, 4) == b"send"
        compressed = gzip.compress(b"decoded")
        connection.sendall(
            b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: "
            + str(len(compressed)).encode()
            + b"\r\n\r\n"
            + compressed[:-8]
        )
    closed(connection)


def serve_http2(connection, mode):
    if mode == "h2c":
        fields = h1_request(connection, b"POST / HTTP/1.1")
        assert fields[b"upgrade"].lower() == b"h2c"
        assert {b"upgrade", b"http2-settings"} <= {
            value.strip().lower() for value in fields[b"connection"].split(b",")
        }
        settings = fields[b"http2-settings"]
        assert len(base64.urlsafe_b64decode(settings + b"=" * (-len(settings) % 4))) % 6 == 0
        assert fields[b"content-length"] == b"4" and exact(connection, 4) == b"send"
        connection.sendall(
            b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: h2c\r\n\r\n"
        )
        begin(connection)
    else:
        begin(connection, mode == "h2-write")
        if mode == "h2-write":
            h2_request(connection, 1, end=False)
            connection.sendall(encoded(1, 4, 1, b"\x88"))
        else:
            h2_requests(connection, {1, 3})
            connection.sendall(encoded(1, 4, 3, b"\x88"))
    reset_with_traffic(connection, mode == "h2-isolation")
    next_stream = 3
    if mode == "h2-isolation":
        connection.sendall(encoded(0, 1, 3))
        next_stream = 5
    h2_request(connection, next_stream)
    connection.sendall(encoded(1, 5, next_stream, b"\x89"))
    closed(connection, h2=True)


def serve(connection, mode):
    connection.settimeout(60)
    connection.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    if mode.startswith("h1-"):
        serve_http1(connection, mode)
    else:
        serve_http2(connection, mode)
