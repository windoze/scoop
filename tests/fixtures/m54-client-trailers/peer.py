import gzip
import os
import socket
import subprocess
import sys
import threading

from wire import chunked, closed, request, response, upgrade


def serve(connection, mode):
    first, headers = request(connection)
    assert first == b"POST / HTTP/1.1"
    assert b"content-length" not in headers and headers[b"transfer-encoding"] == b"chunked"
    assert headers[b"trailer"] == b"x-plain-bytes, x-end"
    if mode in ("h2c", "reject"):
        assert headers[b"expect"] == b"100-continue"
    if mode == "reject":
        response(connection, 417, b"no")
        closed(connection)
        return
    if mode == "h2c":
        assert headers[b"upgrade"] == b"h2c"
        connection.sendall(b"HTTP/1.1 100 Continue\r\n\r\n")
    body, trailers = chunked(connection)
    if mode == "compressed":
        assert headers[b"content-encoding"] == b"gzip"
        body = gzip.decompress(body)
    else:
        assert b"content-encoding" not in headers
    assert body == (b"" if mode == "empty" else "请求".encode())
    assert trailers == [
        (b"x-plain-bytes", str(len(body)).encode()),
        (b"x-end", b"one"),
        (b"x-end", b"two"),
    ]
    if mode == "h2c":
        upgrade(connection)
    else:
        response(connection)
    closed(connection, h2=mode == "h2c")


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
            assert not thread.is_alive(), "trailer peer did not finish"
        if failures:
            raise failures[0]
        assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
        assert result.stdout == f"trailers {mode}\n" and not result.stderr


for mode in ("h1", "compressed", "empty", "h2c", "reject"):
    exercise(sys.argv[1], mode)
print("independent chunked gzip Continue and h2c trailers passed")
result = subprocess.run(
    [sys.argv[1], "h2"],
    capture_output=True,
    text=True,
    timeout=180,
    env=os.environ,
)
assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
assert result.stdout == "trailers h2\n" and not result.stderr
print("parallel HTTP2 trailers passed")
