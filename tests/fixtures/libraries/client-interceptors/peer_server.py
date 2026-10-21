import base64
import gzip
import hashlib
import socketserver
import threading
import traceback

from websocket_wire import frame, read_frame


class Peer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True

    def __init__(self, mode):
        super().__init__(("127.0.0.1", 0), Handler)
        self.mode = mode
        self.errors = []
        self.requests = []
        self.keys = []
        self.abandoned = threading.Event()
        self.lock = threading.Lock()


class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        try:
            self.connection.settimeout(30)
            self.request_loop()
        except Exception:
            self.server.errors.append(traceback.format_exc())

    def response(self, status=200, body=b"", fields=()):
        head = [f"HTTP/1.1 {status} Result\r\nContent-Length: {len(body)}\r\n".encode()]
        head.extend(f"{name}: {value}\r\n".encode() for name, value in fields)
        self.wfile.write(b"".join(head) + b"\r\n" + body)

    def body(self, fields):
        if fields.get("transfer-encoding") != "chunked":
            return self.rfile.read(int(fields.get("content-length", "0")))
        result = bytearray()
        while count := int(self.rfile.readline().strip(), 16):
            result.extend(self.rfile.read(count))
            assert self.rfile.read(2) == b"\r\n"
        assert self.rfile.readline() == b"\r\n"
        return bytes(result)

    def request_loop(self):
        while line := self.rfile.readline():
            method, target, version = line.decode().strip().split()
            fields = {}
            while (line := self.rfile.readline()) != b"\r\n":
                assert line
                name, value = line.decode().split(":", 1)
                name = name.lower()
                assert name not in fields, name
                fields[name] = value.strip()
            body = self.body(fields)
            with self.server.lock:
                self.server.requests.append(target)
                attempt = len(self.server.requests)
            assert version == "HTTP/1.1"
            mode = self.server.mode
            if target == "/probe":
                if mode in ("replace", "throw", "owner", "kind"):
                    assert self.server.abandoned.wait(5), "undelivered response remained open"
                self.response(body=b"probe")
            elif mode == "chain":
                self.chain(method, target, fields, body, attempt)
            elif mode == "cache":
                token = fields["authorization"].removeprefix("Bearer ")
                self.response(
                    body=f"{token}:{fields['x-variant']}".encode(),
                    fields=[("Cache-Control", "private, max-age=3600"), ("Vary", "X-Variant")],
                )
            elif mode == "auth":
                assert fields["authorization"] == "Bearer explicit"
                self.response(401, fields=[("WWW-Authenticate", 'Basic realm="test"')])
            elif mode == "cache-refresh":
                assert fields["x-attempt"] == ("2" if attempt == 3 else "1")
                if attempt == 1:
                    self.response(
                        body=b"old", fields=[("Cache-Control", "no-cache"), ("ETag", '"old"')]
                    )
                elif attempt == 2:
                    assert fields["if-none-match"] == '"old"'
                    self.wfile.write(b'HTTP/1.1 304 Not Modified\r\nETag: "changed"\r\n\r\n')
                else:
                    assert "if-none-match" not in fields
                    self.response(body=b"new", fields=[("Cache-Control", "max-age=3600")])
            elif mode == "proxy":
                assert target == "http://origin.invalid/private?x=%2F"
                assert fields["x-signature"] == hashlib.sha256(target.encode()).hexdigest()
                if attempt == 1:
                    assert "proxy-authorization" not in fields
                    self.response(407, fields=[("Proxy-Authenticate", 'Basic realm="proxy"')])
                else:
                    scheme, token = fields["proxy-authorization"].split()
                    assert (
                        scheme.lower() == "basic" and base64.b64decode(token) == b"proxy:password"
                    )
                    self.response(body=b"proxy")
            elif mode == "websocket":
                if self.websocket(target, fields, attempt):
                    return
            elif target == "/start" and mode == "once":
                self.response(body=b"once")
            elif target == "/short" and mode == "short":
                self.response(body=b"short")
            elif target == "/replacement" and mode == "replace":
                self.response(body=b"replacement")
            elif (target == "/start" and mode in ("replace", "throw", "kind")) or (
                target == "/other" and mode == "owner"
            ):
                self.wfile.write(b"HTTP/1.1 200 OK\r\nContent-Length: 32\r\n\r\n")
                assert not self.rfile.read(1), "unexpected data after undelivered response"
                self.server.abandoned.set()
                return
            else:
                raise AssertionError((mode, target))

    def chain(self, method, target, fields, body, attempt):
        assert fields["x-inner"] == "yes"
        assert fields["x-attempt"] == str(attempt)
        signed = f"{method} {target} {attempt}".encode()
        assert fields["x-signature"] == hashlib.sha256(signed).hexdigest()
        if attempt == 1:
            assert method == "POST" and target == "/start?x=%2F"
            assert fields["content-encoding"] == "gzip"
            assert gzip.decompress(body) == "payload 雪".encode()
            self.response(303, fields=[("Location", "/final?x=%2f")])
        else:
            assert method == "GET" and target == "/final?x=%2F" and not body
            assert "content-encoding" not in fields and "content-type" not in fields
            if attempt == 2:
                self.response(503)
            elif attempt == 3:
                self.response(401, fields=[("WWW-Authenticate", 'Basic realm="test"')])
            else:
                scheme, token = fields["authorization"].split()
                assert scheme.lower() == "basic" and base64.b64decode(token) == b"user:password"
                self.response(body=b"done")

    def websocket(self, target, fields, attempt):
        assert fields["x-application"] == "ws"
        key = fields["sec-websocket-key"]
        assert key not in self.server.keys
        self.server.keys.append(key)
        assert fields["x-signature"] == hashlib.sha256(key.encode()).hexdigest()
        if attempt == 1:
            assert target == "/ws"
            self.response(302, fields=[("Location", "/ws-final")])
            return False
        assert target == "/ws-final"
        accept = base64.b64encode(
            hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()
        )
        self.wfile.write(
            b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: "
            + accept
            + b"\r\n\r\n"
            + frame(1, b"intercepted")
        )
        code, payload = read_frame(self.rfile)
        assert code & 15 == 8
        self.wfile.write(frame(8, payload))
        return True
