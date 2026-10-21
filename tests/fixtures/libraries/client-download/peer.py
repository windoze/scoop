import base64
import collections
import contextlib
import gzip
import http.server
import os
import subprocess
import sys
import threading
from pathlib import Path

PAYLOAD = "explicit destination 中文".encode()
ENCODED = gzip.compress(PAYLOAD, mtime=0)
MODES = (
    "plain",
    "gzip",
    "raw",
    "empty",
    "status",
    "replace",
    "exists",
    "truncated",
    "bad-gzip",
    "redirect",
    "auth",
    "cache",
    "chunked",
    "range",
    "long",
)
COUNTS = collections.Counter()
FAILURES = []
TIMEOUT = 600 if os.environ.get("SCOOP_GC_STRESS_MOVE") == "1" else 180


class Peer(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def handle(self):
        with contextlib.suppress(ConnectionResetError):
            super().handle()

    def do_GET(self):
        try:
            self.respond()
        except (BrokenPipeError, ConnectionResetError):
            pass
        except BaseException as error:
            FAILURES.append(error)
            self.close_connection = True

    def respond(self):
        mode = self.path[1:]
        COUNTS[mode] += 1
        expected = (
            "gzip"
            if mode == "raw"
            else "identity"
            if mode == "range"
            else "gzip, deflate, br, zstd"
        )
        assert self.headers.get("Accept-Encoding") == expected
        if mode == "redirect":
            self.head(302, b"", [("Location", "/redirected"), ("Set-Cookie", "next=yes; Path=/")])
            return
        if mode == "redirected":
            assert self.headers.get("Cookie") == "next=yes"
        if mode == "auth" and self.headers.get("Authorization") is None:
            self.head(401, b"", [("WWW-Authenticate", 'Basic realm="download"')])
            return
        if mode == "auth":
            scheme, token = self.headers["Authorization"].split(" ", 1)
            assert scheme.lower() == "basic"
            assert token == base64.b64encode(b"user:password").decode()
        fields = [("Content-Disposition", 'attachment; filename="../ignored"')]
        body = PAYLOAD
        status = 200
        if mode in ("gzip", "raw", "bad-gzip"):
            fields.append(("Content-Encoding", "gzip"))
            body = ENCODED if mode != "bad-gzip" else ENCODED[:-4]
        if mode == "long":
            body = b"0123456789abcdef" * 4096 + b"!"
        if mode == "empty":
            body, status = b"", 204
        if mode == "status":
            body, status = b"error", 404
        if mode == "range":
            assert self.headers.get("Range") == "bytes=2-4"
            body, status = b"234", 206
            fields.append(("Content-Range", "bytes 2-4/10"))
        if mode == "cache":
            fields.append(("Cache-Control", "max-age=3600"))
        if mode == "truncated":
            self.send_response(200)
            self.send_header("Content-Length", str(len(body) + 100))
            self.end_headers()
            self.wfile.write(body)
            self.close_connection = True
            return
        if mode == "chunked":
            self.send_response(200)
            self.send_header("Transfer-Encoding", "chunked")
            self.send_header("Trailer", "Checksum")
            self.end_headers()
            for index in range(0, len(body), 3):
                part = body[index : index + 3]
                self.wfile.write(f"{len(part):x}\r\n".encode() + part + b"\r\n")
            self.wfile.write(b"0\r\nChecksum: complete\r\n\r\n")
            return
        self.head(status, body, fields)

    def head(self, status, body, fields):
        self.send_response(status)
        if status != 204:
            self.send_header("Content-Length", str(len(body)))
        for name, value in fields:
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(body)


binary, root = sys.argv[1], Path(sys.argv[2])
root.mkdir(parents=True, exist_ok=True)
with http.server.ThreadingHTTPServer(("127.0.0.1", 0), Peer) as server:
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        for mode in MODES:
            directory = root / mode
            directory.mkdir()
            result = subprocess.run(
                [binary, mode, f"http://127.0.0.1:{server.server_port}/{mode}", str(directory)],
                capture_output=True,
                text=True,
                timeout=TIMEOUT,
                env=os.environ,
            )
            assert not FAILURES, (mode, FAILURES, result.stdout, result.stderr)
            assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
            assert result.stdout == f"client download {mode}\n" and not result.stderr
            expected = PAYLOAD
            if mode == "raw":
                expected = ENCODED
            elif mode == "empty":
                expected = b""
            elif mode in ("status", "exists", "truncated", "bad-gzip"):
                expected = b"original"
            elif mode == "range":
                expected = b"234"
            elif mode == "long":
                expected = b"0123456789abcdef" * 4096 + b"!"
            assert (directory / "result").read_bytes() == expected, mode
            assert sorted(p.name for p in directory.iterdir()) == (
                ["cached", "result"] if mode == "cache" else ["result"]
            )
        assert not FAILURES, FAILURES
        assert (
            collections.Counter({**dict.fromkeys(MODES, 1), "auth": 2, "redirected": 1}) == COUNTS
        ), COUNTS
    finally:
        server.shutdown()
        worker.join(TIMEOUT)
directory = root / "http2"
directory.mkdir()
result = subprocess.run(
    [binary, "http2", str(directory)],
    capture_output=True,
    text=True,
    timeout=TIMEOUT,
    env=os.environ,
)
assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
assert result.stdout == "client download http2\n" and not result.stderr
print("client downloads independent file bytes protocol composition and parallel HTTP2 passed")
