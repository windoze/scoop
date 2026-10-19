import gzip
import os
import socket
import subprocess
import sys
import threading
import zlib

from wire import chunked_response, closed, request, response


def serve_data(connection):
    request(connection, b"/gzip")
    response(connection, gzip.compress("来自 Python".encode()), [(b"Content-Encoding", b"gzip")])
    request(connection, b"/stacked")
    chunked_response(connection, zlib.compress(gzip.compress("多层内容".encode())))
    request(connection, b"/latin")
    response(connection, b"caf\xe9", [(b"Content-Type", b"text/plain; charset=ISO-8859-1")])
    request(connection, b"/json")
    response(
        connection,
        b"\xef\xbb\xbf" + '"中文"'.encode(),
        [(b"Content-Type", b"application/json; charset=ISO-8859-1")],
    )
    request(connection, b"/override")
    response(
        connection,
        b"caf\xe9",
        [(b"Content-Type", b"text/plain"), (b"content-type", b"application/octet-stream")],
    )
    closed(connection)


def serve_policy(connection):
    fields = request(connection, b"/raw", accept=b"gzip")
    assert fields[b"range"] == b"bytes=0-2"
    response(connection, b"raw", [(b"Content-Encoding", b"unsupported / coding")])
    fields = request(connection, b"/range", accept=b"identity")
    assert fields[b"range"] == b"bytes=0-2"
    response(connection, b"abc", [(b"Content-Encoding", b"identity")])
    request(connection, b"/head", method=b"HEAD")
    response(connection, fields=[(b"Content-Encoding", b"gzip"), (b"Content-Length", b"123")])
    request(connection, b"/not-modified")
    response(
        connection,
        fields=[(b"Content-Encoding", b"unknown / metadata"), (b"Content-Length", b"123")],
        status=304,
    )
    request(connection, b"/limit")
    response(connection, gzip.compress(b"abcdef"), [(b"Content-Encoding", b"gzip")])
    request(connection, b"/truncated")
    response(connection, gzip.compress(b"truncated")[:-4], [(b"Content-Encoding", b"gzip")])
    closed(connection)


def exercise(binary, mode, serve):
    failures = []
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(4)
        listener.settimeout(240)

        def worker():
            try:
                with listener.accept()[0] as connection:
                    connection.settimeout(240)
                    serve(connection)
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
            timeout=240,
            env=dict(os.environ, SCOOP_GC_STRESS_MOVE="1"),
        )
        for thread in workers:
            thread.join(240)
            assert not thread.is_alive(), "response peer did not finish"
        if failures:
            raise failures[0]
        assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
        assert result.stdout == f"response {mode}\n" and not result.stderr


exercise(sys.argv[1], "data", serve_data)
exercise(sys.argv[1], "policy", serve_policy)
print("independent HTTP response decoding passed")
result = subprocess.run(
    [sys.argv[1], "h2"],
    capture_output=True,
    text=True,
    timeout=240,
    env=dict(os.environ, SCOOP_GC_STRESS_MOVE="1"),
)
assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
assert result.stdout == "response h2\n" and not result.stderr
print("parallel HTTP2 content decoding passed")
