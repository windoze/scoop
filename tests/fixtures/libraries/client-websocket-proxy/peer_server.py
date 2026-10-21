import base64
import hashlib
import ipaddress
import socketserver
import traceback

import http_wire
from tls_peer import TlsLayer
from wire import frame, message, read_frame


class ProxyPeer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, mode, context):
        super().__init__(("127.0.0.1", 0), ProxyHandler)
        self.mode, self.context = mode, context
        self.errors = []
        self.connections = 0
        self.upgrades = 0


class SocketReader:
    def __init__(self, connection):
        self.connection = connection

    def read(self, size):
        return http_wire.exact(self.connection, size)


class ProxyHandler(socketserver.BaseRequestHandler):
    def handle(self):
        try:
            self.exchange()
        except Exception:
            self.server.errors.append(traceback.format_exc())

    def exchange(self):
        peer, connection = self.server, self.request
        connection.settimeout(30)
        peer.connections += 1
        mode = peer.mode
        if mode == "secure-connect":
            connection = TlsLayer(connection, peer.context)
            assert connection.ssl.selected_alpn_protocol() == "http/1.1"
        if mode.startswith("socks-"):
            self.socks(connection)
        else:
            first, fields = http_wire.request(connection)
            assert first == b"CONNECT localhost:9443 HTTP/1.1", first
            assert fields[b"host"] == b"localhost:9443"
            assert b"authorization" not in fields and b"cookie" not in fields
            if mode == "connect-auth" and peer.connections == 1:
                assert b"proxy-authorization" not in fields
                connection.sendall(
                    b"HTTP/1.1 407 Challenge\r\nConnection: close\r\nContent-Length: 0\r\n"
                    b'Proxy-Authenticate: Basic realm="proxy"\r\n\r\n'
                )
                http_wire.closed(connection)
                return
            scheme, token = fields[b"proxy-authorization"].split(b" ", 1)
            assert scheme.lower() == b"basic" and token == base64.b64encode(b"proxy:secret")
            connection.sendall(
                b"HTTP/1.1 103 Early Hints\r\nLink: </proxy-only>\r\n\r\n"
                b"HTTP/1.1 200 Established\r\n\r\n"
            )
        if not mode.startswith("socks-") or mode == "socks-wss":
            connection = TlsLayer(connection, peer.context)
            assert connection.ssl.selected_alpn_protocol() == "http/1.1"
        self.websocket(connection)

    def socks(self, connection):
        mode = self.server.mode
        auth = mode == "socks-auth"
        assert http_wire.exact(connection, 3) == bytes((5, 1, 2 if auth else 0))
        connection.sendall(bytes((5, 2 if auth else 0)))
        if auth:
            assert http_wire.exact(connection, 1) == b"\x01"
            username = http_wire.exact(connection, http_wire.exact(connection, 1)[0])
            password = http_wire.exact(connection, http_wire.exact(connection, 1)[0])
            assert username == "代理".encode() and password == b"secret\0"
            connection.sendall(b"\x01\0")
        head = http_wire.exact(connection, 4)
        assert head[:3] == b"\x05\x01\0"
        kind = head[3]
        count = http_wire.exact(connection, 1)[0] if kind == 3 else {1: 4, 4: 16}[kind]
        address = http_wire.exact(connection, count)
        assert int.from_bytes(http_wire.exact(connection, 2), "big") == 9443
        if mode == "socks-local":
            assert kind in (1, 4) and ipaddress.ip_address(address).is_loopback
        else:
            assert kind == 3 and address == b"localhost"
        connection.sendall(b"\x05\0\0\x01\x7f\0\0\x01\x10\xe1")

    def websocket(self, connection):
        first, fields = http_wire.request(connection)
        assert first == b"GET /resource?q=%2F HTTP/1.1", first
        assert fields[b"host"] == b"localhost:9443"
        assert fields[b"authorization"] == b"Bearer origin"
        assert fields[b"cookie"] == b"origin=cookie"
        assert b"proxy-authorization" not in fields
        assert fields[b"upgrade"].lower() == b"websocket"
        assert b"content-length" not in fields and b"transfer-encoding" not in fields
        key = fields[b"sec-websocket-key"]
        assert len(base64.b64decode(key)) == 16
        self.server.upgrades += 1
        accept = base64.b64encode(
            hashlib.sha1(key + b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11").digest()
        )
        connection.sendall(
            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
            b"Sec-WebSocket-Accept: " + accept + b"\r\n\r\n" + frame(1, b"tunneled")
        )
        reader = SocketReader(connection)
        opcode, payload, compressed = message(reader)
        assert opcode == 1 and payload == b"through proxy" and not compressed
        connection.sendall(frame(1, b"echo:" + payload))
        code, payload = read_frame(reader)
        assert code & 15 == 8 and payload == b"\x03\xe8"
        connection.sendall(frame(8, payload))
