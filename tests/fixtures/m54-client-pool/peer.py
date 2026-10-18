import os
import socket
import struct
import subprocess
import sys
import threading


def exact(connection, count):
    result = bytearray()
    while len(result) < count:
        part = connection.recv(count - len(result))
        if not part:
            raise AssertionError("unexpected peer EOF")
        result.extend(part)
    return bytes(result)


def closed(connection):
    try:
        value = connection.recv(1)
    except ConnectionResetError:
        return
    assert not value, value


def request(connection):
    header = bytearray()
    while not header.endswith(b"\r\n\r\n"):
        header.extend(exact(connection, 1))
        assert len(header) <= 8192
    lines = bytes(header).split(b"\r\n")
    assert lines[0].startswith(b"GET ") and lines[0].endswith(b" HTTP/1.1")
    return lines[0].split(b" ")[1]


def response(connection, text):
    body = text.encode()
    connection.sendall(
        b"HTTP/1.1 200 Independent peer\r\nContent-Length: "
        + str(len(body)).encode()
        + b"\r\n\r\n"
        + body
    )


def http1(listener):
    with listener.accept()[0] as connection:
        connection.settimeout(180)
        for index in range(3):
            assert request(connection) == f"/reuse/{index}".encode()
            response(connection, str(index))
        assert request(connection) == b"/abandon"
        response(connection, "not consumed")
        closed(connection)
    with listener.accept()[0] as connection:
        connection.settimeout(180)
        assert request(connection) == b"/new"
        response(connection, "new")
        closed(connection)


def send_frame(connection, kind, flags, stream, payload=b""):
    connection.sendall(
        len(payload).to_bytes(3, "big") + bytes((kind, flags)) + struct.pack("!I", stream) + payload
    )


def frame(connection):
    header = exact(connection, 9)
    length = int.from_bytes(header[:3], "big")
    assert length <= 65536
    return header[3], header[4], int.from_bytes(header[5:], "big"), exact(connection, length)


def begin_http2(connection):
    connection.settimeout(180)
    assert exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    send_frame(connection, 4, 0, 0, struct.pack("!HI", 3, 2))


def next_request(connection):
    while True:
        kind, flags, stream, payload = frame(connection)
        if kind == 4:
            assert stream == 0
            if not flags & 1:
                send_frame(connection, 4, 1, 0)
        elif kind == 1:
            assert flags & 4
            return stream
        else:
            assert kind in (0, 8), (kind, flags, stream, payload)


def h2_response(connection, stream, text, end=False):
    send_frame(connection, 1, 4, stream, b"\x88")
    send_frame(connection, 0, 1 if end else 0, stream, text.encode())


def http2(listener):
    with listener.accept()[0] as connection:
        begin_http2(connection)
        assert next_request(connection) == 1
        h2_response(connection, 1, "a")
        assert next_request(connection) == 3
        send_frame(connection, 4, 0, 0, struct.pack("!HI", 3, 1))
        h2_response(connection, 3, "b")
        resets = set()
        while resets != {1, 3}:
            kind, flags, stream, payload = frame(connection)
            assert kind != 1, "new stream was sent before both occupied slots were released"
            if kind == 3:
                assert stream in (1, 3) and payload == struct.pack("!I", 8)
                resets.add(stream)
            elif kind == 4 and not flags & 1:
                send_frame(connection, 4, 1, 0)
            else:
                assert kind in (0, 4, 8), (kind, flags, stream)
        assert next_request(connection) == 5
        send_frame(connection, 7, 0, 0, struct.pack("!II", 5, 0))
        h2_response(connection, 5, "third", end=True)
        # The client can flush WINDOW_UPDATE before closing the drained connection.
        while True:
            try:
                first = connection.recv(1)
            except ConnectionResetError:
                break
            if not first:
                break
            header = first + exact(connection, 8)
            payload = exact(connection, int.from_bytes(header[:3], "big"))
            assert header[3] in (0, 3, 4, 7, 8), (header, payload)
    with listener.accept()[0] as connection:
        begin_http2(connection)
        assert next_request(connection) == 1
        h2_response(connection, 1, "next", end=True)
        # Keep the transport alive until the client's explicit close.
        while connection.recv(4096):
            pass


def exercise(binary, name, serve):
    failures = []
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(4)
        listener.settimeout(180)

        def worker():
            try:
                serve(listener)
            except BaseException as error:
                failures.append(error)

        thread = threading.Thread(target=worker, daemon=True)
        thread.start()
        address = f"http://127.0.0.1:{listener.getsockname()[1]}/"
        result = subprocess.run(
            [binary, name, address],
            capture_output=True,
            text=True,
            timeout=180,
            env=dict(os.environ, SCOOP_GC_STRESS_MOVE="1"),
        )
        thread.join(180)
        assert not thread.is_alive(), "HTTP pool peer did not finish"
        if failures:
            raise failures[0]
        assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
        assert result.stdout == f"pooled {name}\n" and not result.stderr
    print(f"independent {name} pool peer passed")


exercise(sys.argv[1], "h1", http1)
exercise(sys.argv[1], "h2", http2)
