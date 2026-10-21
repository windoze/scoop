import socket
import struct
import subprocess
import sys
import threading

from wire import begin, closed, frame_bytes, request


def exercise(binary, mode):
    failures = []
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(4)
        listener.settimeout(30)

        def worker():
            try:
                with listener.accept()[0] as connection:
                    connection.settimeout(30)
                    begin(connection, False)
                    request(connection, 1)
                    if mode == "refused":
                        request(connection, 3)
                    connection.sendall(
                        frame_bytes(7, 0, 0, struct.pack("!II", 1, 0))
                        + frame_bytes(1, 4, 1, b"\x88")
                        + frame_bytes(0, 1, 1, b"kept")
                    )
                    closed(connection, h2=True)
            except BaseException as error:
                failures.append(error)

        workers = [threading.Thread(target=worker, daemon=True) for _ in range(2)]
        for worker_thread in workers:
            worker_thread.start()
        result = subprocess.run(
            [binary, mode, str(listener.getsockname()[1])],
            capture_output=True,
            text=True,
            timeout=90,
        )
        for worker_thread in workers:
            worker_thread.join(30)
            assert not worker_thread.is_alive(), "HTTP/2 peer did not finish"
        assert result.returncode == 0, (
            mode,
            result.returncode,
            result.stdout,
            result.stderr,
            failures,
        )
        assert not failures, failures
        assert result.stdout == f"HTTP/2 draining {mode}\n" and not result.stderr


for case in ("kept", "refused"):
    exercise(sys.argv[1], case)
print("independent HTTP2 draining and unprocessed streams passed")
