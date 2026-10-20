import socket
import subprocess
import sys
import threading
import traceback

from wire import begin, closed, encoded, frame, h2_request


def gate(connection, value):
    while True:
        kind, flags, stream, payload = frame(connection)
        if kind == 0:
            assert stream == 3 and payload == bytes([value]) and flags == 0
            return
        assert kind in (4, 8), (kind, flags, stream, payload)


def serve(listener, errors):
    try:
        connection, _ = listener.accept()
        with connection:
            connection.settimeout(30)
            begin(connection)
            h2_request(connection, 1)
            h2_request(connection, 3, end=False)
            partial = encoded(1, 0, 1, b"\x88")
            connection.sendall(encoded(1, 5, 3, b"\x88") + partial[:8])
            gate(connection, 1)
            connection.sendall(partial[8:])
            gate(connection, 2)
            while True:
                kind, flags, stream, payload = frame(connection)
                if kind in (0, 1):
                    assert stream == 3 and flags == (1 if kind == 0 else 5) and not payload
                    break
                assert kind in (4, 8), (kind, flags, stream, payload)
            connection.sendall(encoded(9, 4, 1) + encoded(0, 1, 1, b"done"))
            closed(connection, h2=True)
    except Exception:
        errors.append(traceback.format_exc())


def run(executable, mode, port=None):
    args = [executable, mode] + ([] if port is None else [str(port)])
    result = subprocess.run(args, capture_output=True, text=True, timeout=60)
    assert result.returncode == 0, (mode, result.stdout, result.stderr)
    assert result.stdout == f"HTTP response timing case passed: {mode}\n"
    assert not result.stderr, result.stderr


def main():
    run(sys.argv[1], "http1")
    for mode in ("on", "off"):
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            listener.settimeout(30)
            errors = []
            thread = threading.Thread(target=serve, args=(listener, errors), daemon=True)
            thread.start()
            try:
                run(sys.argv[1], mode, listener.getsockname()[1])
            finally:
                thread.join(35)
            assert not thread.is_alive()
            assert not errors, errors
    print(
        "HTTP response first-byte timing preserves partial headers pipeline and stream attribution"
    )


if __name__ == "__main__":
    main()
