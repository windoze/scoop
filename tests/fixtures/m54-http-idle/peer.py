import os
import select
import socket
import struct
import subprocess
import sys
import threading
import time

from wire import begin, closed, frame, frame_bytes, h1_request, request, string


def paced(connection, wire, capacity):
    for offset in range(0, len(wire), capacity):
        connection.sendall(wire[offset : offset + capacity])
        time.sleep(0.2)


def reset_with_other_traffic(connection, writing):
    # Only the timed-out stream's own bytes may reset its clock.
    # Bound pending probes so GC pauses cannot accumulate unrelated control work.
    pending_ping = False
    probe_seen = False
    reset_seen = False
    for _ in range(80):
        if not pending_ping and not reset_seen:
            connection.sendall(frame_bytes(6, 0, 0, b"12345678"))
            if probe_seen and not writing:
                connection.sendall(frame_bytes(0, 0, 3, b"x"))
            pending_ping = True
        ready = select.select([connection], [], [], 0.08)[0]
        while ready:
            kind, flags, stream, payload = frame(connection)
            if kind == 1:
                assert not probe_seen and stream == 3 and flags == 5
                probe_seen = True
                if not writing:
                    connection.sendall(frame_bytes(1, 4, 3, b"\x88") + frame_bytes(0, 0, 3, b"x"))
            elif kind == 3:
                assert not reset_seen and stream == 1 and payload == struct.pack("!I", 8)
                reset_seen = True
            elif kind == 6:
                assert flags == 1 and stream == 0 and payload == b"12345678"
                pending_ping = False
            else:
                assert kind in (4, 8), (kind, flags, stream, payload)
            # Cancellation and a later request may arrive in either order.
            if probe_seen and reset_seen:
                return
            ready = select.select([connection], [], [], 0)[0]
        time.sleep(0.04)
    raise AssertionError("background stream idle timer did not send CANCEL")


def serve(connection, mode):
    connection.settimeout(15)
    connection.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    if mode.startswith("h1-"):
        h1_request(connection)
        if mode == "h1-stall":
            connection.sendall(b"HTTP/1.1 200 Partial")
        else:
            header = b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nx-pad: " + b"p" * 144 + b"\r\n\r\n"
            paced(connection, header, 12)
            paced(connection, b"abc", 1)
        closed(connection)
        return
    writing = mode == "h2-write"
    begin(connection, writing)
    request(connection, 1, end=not writing and mode != "h2-pause")
    if mode == "h2-progress":
        block = b"\x88\x00" + string(b"x-pad") + string(b"p" * 900)
        connection.sendall(frame_bytes(1, 1, 1, block[:31]))
        continuation = frame_bytes(9, 4, 1, block[31:])
        connection.sendall(continuation[:9])
        paced(connection, continuation[9:], 64)
    elif mode == "h2-pause":
        body = bytearray()
        while True:
            kind, flags, stream, payload = frame(connection)
            if kind == 0:
                assert stream == 1 and not flags & 8
                body.extend(payload)
                if flags & 1:
                    break
            elif kind == 1:
                # The upload may end with empty trailing HEADERS.
                assert stream == 1 and flags == 5 and not payload
                break
            else:
                assert kind in (4, 6, 8), (kind, flags, stream, payload)
        assert body == b"ab", body
        connection.sendall(frame_bytes(1, 5, 1, b"\x89"))
    else:
        reset_with_other_traffic(connection, writing)
        if writing:
            connection.sendall(frame_bytes(1, 5, 3, b"\x89"))
        else:
            connection.sendall(frame_bytes(0, 1, 3))
            request(connection, 5)
            connection.sendall(frame_bytes(1, 5, 5, b"\x89"))
    closed(connection, h2=True)


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
            env=dict(os.environ, SCOOP_GC_STRESS_MOVE="1"),
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
        assert result.stdout == f"{mode} idle passed\n" and not result.stderr


modes = sys.argv[2:] or (
    "h1-progress",
    "h1-stall",
    "h2-progress",
    "h2-isolation",
    "h2-write",
    "h2-pause",
)
for mode in modes:
    exercise(sys.argv[1], mode)
print("independent HTTP idle progress and stream isolation passed")
