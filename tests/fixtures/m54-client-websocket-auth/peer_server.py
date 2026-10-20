import base64
import hashlib
import socketserver
import threading
import traceback

from http_wire import closed, request
from oracle import challenge, parameters, verify
from wire import frame, read_frame


def credential(value, scheme, token):
    actual_scheme, actual_token = value.split(b" ", 1)
    assert actual_scheme.lower() == scheme and actual_token == token, value


class AuthPeer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, mode, context=None):
        super().__init__(("127.0.0.1", 0), AuthHandler)
        self.mode = mode
        self.context = context
        self.proxy = "proxy-" in mode
        self.keys = []
        self.errors = []
        self.lock = threading.Lock()

    def get_request(self):
        connection, address = super().get_request()
        connection.settimeout(30)
        if self.context:
            connection = self.context.wrap_socket(connection, server_side=True)
            assert connection.selected_alpn_protocol() == "http/1.1"
        return connection, address


class AuthHandler(socketserver.BaseRequestHandler):
    def handle(self):
        try:
            self.exchange()
        except Exception:
            self.server.errors.append(traceback.format_exc())

    def exchange(self):
        connection, peer = self.request, self.server
        first, fields = request(connection)
        target = b"/resource?q=%2F"
        if peer.proxy:
            target = b"http://destination.invalid:9876" + target
        assert first == b"GET " + target + b" HTTP/1.1", first
        assert fields[b"upgrade"].lower() == b"websocket"
        assert b"upgrade" in fields[b"connection"].lower()
        assert fields[b"sec-websocket-version"] == b"13"
        assert b"content-length" not in fields and b"transfer-encoding" not in fields
        key = fields[b"sec-websocket-key"]
        assert len(base64.b64decode(key)) == 16
        with peer.lock:
            assert key not in peer.keys
            peer.keys.append(key)
            attempt = len(peer.keys)
        name = b"proxy-authorization" if peer.proxy else b"authorization"
        assert (b"authorization" if peer.proxy else b"proxy-authorization") not in fields
        mode = peer.mode
        digest = mode in ("digest", "integrity", "bad-proof") or mode.endswith("-digest")
        bearer = mode in ("bearer", "refresh", "proxy-bearer")
        qop = "auth" if mode == "digest" else "auth-int"
        algorithm = "SHA-256-sess"
        connection.sendall(b"HTTP/1.1 103 Early Hints\r\nLink: </hint>\r\n\r\n")
        if attempt == 1:
            if mode == "refresh":
                credential(fields[name], b"bearer", b"old")
            else:
                assert name not in fields
            value = (
                challenge(algorithm, qop)
                if digest
                else (
                    b'Bearer realm="private", error="invalid_token"'
                    if bearer
                    else b'Basic realm="private"'
                )
            )
            header = b"Proxy-Authenticate" if peer.proxy else b"WWW-Authenticate"
            status = b"407" if peer.proxy else b"401"
            connection.sendall(
                b"HTTP/1.1 "
                + status
                + b" Challenge\r\nConnection: close\r\nContent-Length: 0\r\n"
                + header
                + b": "
                + value
                + b"\r\n\r\n"
            )
            closed(connection)
            return
        assert attempt == 2
        info = b""
        if digest:
            proof = verify(fields[name], b"GET", target, b"", algorithm, qop)
            values = proof(b"")
            if mode == "bad-proof":
                values["rspauth"] = b"0" * 64
            info = b"Proxy-Authentication-Info" if peer.proxy else b"Authentication-Info"
            info += b": " + parameters(values) + b"\r\n"
        elif bearer:
            credential(fields[name], b"bearer", b"new" if mode == "refresh" else b"token")
        else:
            credential(fields[name], b"basic", base64.b64encode(b"user:secret"))
        accept = base64.b64encode(
            hashlib.sha1(key + b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11").digest()
        )
        connection.sendall(
            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
            b"Sec-WebSocket-Accept: "
            + accept
            + b"\r\n"
            + info
            + b"\r\n"
            + frame(1, b"authenticated")
        )
        if mode == "bad-proof":
            closed(connection)
            return
        with connection.makefile("rb") as source:
            code, payload = read_frame(source)
        assert code & 15 == 8 and payload == b"\x03\xe8"
        connection.sendall(frame(8, payload))
