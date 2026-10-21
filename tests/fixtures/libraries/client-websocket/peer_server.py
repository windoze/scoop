import base64
import gzip
import hashlib
import socketserver
import threading
import traceback

from wire import compressed_frame, frame, message, read_frame


class Peer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, mode):
        super().__init__(("127.0.0.1", 0), Handler)
        self.mode = mode
        self.errors = []
        self.keys = []
        self.probes = 0
        self.lock = threading.Lock()


class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        try:
            self.connection.settimeout(30)
            self.request_loop()
        except Exception:
            self.server.errors.append(traceback.format_exc())

    def response(self, status, fields, body=b""):
        head = [f"HTTP/1.1 {status} Test\r\n".encode()]
        head.extend(f"{name}: {value}\r\n".encode() for name, value in fields)
        self.wfile.write(b"".join(head) + b"\r\n" + body)
        self.wfile.flush()

    def request_loop(self):
        while True:
            line = self.rfile.readline()
            if not line:
                return
            method, target, version = line.decode().strip().split()
            fields = {}
            while (line := self.rfile.readline()) != b"\r\n":
                assert line
                name, value = line.decode().split(":", 1)
                name = name.lower()
                assert name not in fields
                fields[name] = value.strip()
            assert method == "GET"
            assert "transfer-encoding" not in fields
            assert fields.get("content-length", "0") == "0"
            mode = self.server.mode
            if target == "/probe":
                assert version == ("HTTP/1.0" if mode == "force-h1" else "HTTP/1.1")
                if mode == "cookie":
                    assert fields.get("cookie") == "token=stored"
                self.server.probes += 1
                self.response(200, [("Content-Length", "5"), ("Connection", "close")], b"probe")
                return
            assert version == "HTTP/1.1"
            assert fields["upgrade"].lower() == "websocket"
            assert "upgrade" in fields["connection"].lower()
            assert fields["sec-websocket-version"] == "13"
            assert fields["sec-websocket-protocol"] == "chat"
            assert fields["x-client"] == "scoop"
            key = fields["sec-websocket-key"]
            assert len(base64.b64decode(key)) == 16
            with self.server.lock:
                assert key not in self.server.keys
                self.server.keys.append(key)
                attempt = len(self.server.keys)
            if attempt == 1 and mode in ("redirect", "ws-redirect", "retry"):
                extra = []
                status = 503 if mode == "retry" else 302
                if status == 302:
                    location = "/final?x=%2f"
                    if mode == "ws-redirect":
                        location = f"ws://127.0.0.1:{self.server.server_address[1]}" + location
                    extra.append(("Location", location))
                self.response(status, [("Content-Length", "0")] + extra)
                continue
            assert target == (
                "/final?x=%2F"
                if mode in ("redirect", "ws-redirect")
                else "/start/%E9%9B%AA?x=%2F&x=2"
            ), target
            if mode == "reject":
                body = gzip.compress(b"handshake denied")
                self.response(
                    403,
                    [
                        ("Content-Encoding", "gzip"),
                        ("Content-Length", str(len(body))),
                        ("Connection", "close"),
                    ],
                    body,
                )
                return
            accept = base64.b64encode(
                hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()
            ).decode()
            response = [
                ("Upgrade", "websocket"),
                ("Connection", "Upgrade"),
                ("Sec-WebSocket-Accept", "wrong" if mode == "bad-accept" else accept),
                ("Sec-WebSocket-Protocol", "other" if mode == "bad-protocol" else "chat"),
            ]
            if mode == "bad-extensions":
                response.append(("Sec-WebSocket-Extensions", "unknown"))
            if mode in ("deflate", "bad-window"):
                bits = 8 if mode == "bad-window" else 15
                response.append(
                    (
                        "Sec-WebSocket-Extensions",
                        f"permessage-deflate; server_no_context_takeover; client_no_context_takeover; client_max_window_bits={bits}; server_max_window_bits=15",
                    )
                )
            if mode == "cookie":
                response.append(("Set-Cookie", "token=stored; Path=/"))
            if mode.startswith("bad-"):
                self.response(101, response)
                return
            ready = "ready 🌍".encode()
            self.response(
                101, response, compressed_frame(ready) if mode == "deflate" else frame(1, ready)
            )
            opcode, payload, compressed = message(self.rfile)
            assert opcode == 1 and payload == "client 🌍".encode()
            assert compressed == (mode == "deflate")
            echo = b"echo:" + payload
            self.wfile.write(compressed_frame(echo) if mode == "deflate" else frame(1, echo))
            code, payload = read_frame(self.rfile)
            assert code & 15 == 9 and payload == b"health"
            self.wfile.write(frame(10, payload))
            opcode, payload, compressed = message(self.rfile)
            assert (
                opcode == 2
                and payload == bytes(index % 256 for index in range(257))
                and not compressed
            )
            self.wfile.write(frame(2, payload))
            code, payload = read_frame(self.rfile)
            assert code & 15 == 8 and payload == b"\x03\xe8finished"
            self.wfile.write(frame(8, payload))
            return
