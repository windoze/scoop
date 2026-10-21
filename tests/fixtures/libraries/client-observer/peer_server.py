import base64
import gzip
import hashlib
import socketserver
import threading
import traceback

from websocket_wire import frame, read_frame


class Peer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, mode):
        super().__init__(("127.0.0.1", 0), Handler)
        self.mode = mode
        self.errors = []
        self.requests = []
        self.connections = 0
        self.lock = threading.Lock()


class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        try:
            self.connection.settimeout(30)
            with self.server.lock:
                self.server.connections += 1
            self.exchange()
        except Exception:
            self.server.errors.append(traceback.format_exc())

    def response(self, status=200, body=b"", fields=()):
        head = [f"HTTP/1.1 {status} Result\r\nContent-Length: {len(body)}\r\n".encode()]
        head.extend(f"{name}: {value}\r\n".encode() for name, value in fields)
        self.wfile.write(b"".join(head) + b"\r\n" + body)

    def body(self, fields):
        if fields.get("transfer-encoding") != "chunked":
            return self.rfile.read(int(fields.get("content-length", "0")))
        body = bytearray()
        while count := int(self.rfile.readline().strip(), 16):
            body.extend(self.rfile.read(count))
            assert self.rfile.read(2) == b"\r\n"
        assert self.rfile.readline() == b"\r\n"
        return bytes(body)

    def exchange(self):
        while line := self.rfile.readline():
            method, target, version = line.decode().strip().split()
            assert version == "HTTP/1.1"
            fields = {}
            while (line := self.rfile.readline()) != b"\r\n":
                assert line
                name, value = line.decode().split(":", 1)
                assert name.lower() not in fields
                fields[name.lower()] = value.strip()
            body = self.body(fields)
            with self.server.lock:
                self.server.requests.append((method, target))
                attempt = len(self.server.requests)
            mode = self.server.mode
            if mode == "h1":
                if attempt == 1:
                    assert method == "POST"
                    assert target == "/private/BODY_SECRET?q=QUERY_SECRET"
                    assert fields["authorization"] == "Bearer HEADER_SECRET"
                    assert fields["cookie"] == "private=HEADER_SECRET"
                    assert gzip.decompress(body) == "BODY_SECRET 雪".encode()
                    self.wfile.write(b"HTTP/1.1 103 Early Hints\r\n\r\n")
                    self.response(
                        body=gzip.compress("response 雪 BODY_SECRET".encode(), mtime=0),
                        fields=[("Content-Encoding", "gzip"), ("X-Received-Bytes", str(len(body)))],
                    )
                else:
                    assert method == "GET" and target == "/reuse" and not body
                    self.response(body=b"reuse")
            elif mode == "follow-up":
                self.follow_up(method, target, fields, body, attempt)
            elif mode == "cache":
                if attempt == 1:
                    self.response(
                        body=b"cached",
                        fields=[("Cache-Control", "private, max-age=3600"), ("ETag", '"one"')],
                    )
                else:
                    assert fields["if-none-match"] == '"one"'
                    self.wfile.write(b'HTTP/1.1 304 Not Modified\r\nETag: "one"\r\n\r\n')
            elif mode == "partial-head":
                self.wfile.write(b"H")
                return
            elif mode == "partial-body":
                self.wfile.write(b"HTTP/1.1 200 OK\r\nContent-Length: 16\r\n\r\nabc")
                return
            elif mode in ("websocket", "websocket-failure", "websocket-total"):
                if target == "/probe":
                    self.response(body=b"probe")
                    continue
                self.websocket(fields, mode != "websocket")
                return
            elif mode == "websocket-rejected":
                self.response(400, b"denied")
            elif mode == "sse":
                self.response(
                    body=b"data: observed\n\n", fields=[("Content-Type", "text/event-stream")]
                )
            else:
                assert mode in ("off", "dns", "tcp-callback")
                self.response(body=b"probe")

    def follow_up(self, method, target, fields, body, attempt):
        if attempt == 1:
            assert method == "POST" and body == b"payload"
            self.response(303, fields=[("Location", "/final")])
        else:
            assert method == "GET" and target == "/final" and not body
            if attempt == 2:
                self.response(503)
            elif attempt == 3:
                self.response(401, fields=[("WWW-Authenticate", 'Basic realm="test"')])
            else:
                scheme, token = fields["authorization"].split()
                assert scheme.lower() == "basic" and base64.b64decode(token) == b"user:secret"
                self.response(body=b"done")

    def websocket(self, fields, abandoned=False):
        key = fields["sec-websocket-key"]
        accept = base64.b64encode(
            hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()
        )
        self.wfile.write(
            b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n"
            b"Sec-WebSocket-Accept: " + accept + b"\r\n\r\n" + frame(1, b"observed")
        )
        if abandoned:
            assert not self.rfile.read(1), "unpublished WebSocket was not closed"
            return
        code, payload = read_frame(self.rfile)
        assert code & 15 == 8
        self.wfile.write(frame(8, payload))
