import socketserver
import ssl
import subprocess
import sys
import threading
import traceback
from pathlib import Path

import h2_wire
import http_wire
from proxy_peer import ProxyPeer
from tls_peer import TlsLayer, contexts


class DirectPeer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, mode, context):
        super().__init__(("127.0.0.1", 0), DirectHandler)
        self.mode, self.context = mode, context
        self.errors = []
        self.connections = 0
        self.requests = 0
        self.failures = 0


class DirectHandler(socketserver.BaseRequestHandler):
    def handle(self):
        peer = self.server
        peer.connections += 1
        self.request.settimeout(30)
        try:
            try:
                connection = TlsLayer(self.request, peer.context)
            except ssl.SSLError:
                assert peer.mode == "tls-failure"
                peer.failures += 1
                return
            if peer.mode == "direct":
                assert connection.ssl.selected_alpn_protocol() == "h2"
                h2_wire.begin(connection)
                for stream in (1, 3):
                    h2_wire.h2_request(connection, stream)
                    peer.requests += 1
                    connection.sendall(
                        h2_wire.encoded(1, 4, stream, b"\x88")
                        + h2_wire.encoded(0, 1, stream, b"body")
                    )
                h2_wire.closed(connection, h2=True)
            else:
                assert peer.mode == "forward"
                first, fields = http_wire.request(connection)
                assert first == b"GET http://origin.invalid/resource?q=QUERY_SECRET HTTP/1.1"
                assert fields[b"host"] == b"origin.invalid"
                peer.requests += 1
                connection.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nbody")
                http_wire.closed(connection)
        except Exception:
            peer.errors.append(traceback.format_exc())


def main():
    binary, source, directory = sys.argv[1:]
    source = Path(source)
    tls = contexts(source / "server.scoop", source / "intermediate.scoop", Path(directory))
    for mode in (
        "direct",
        "forward",
        "connect",
        "secure-connect",
        "connect-auth",
        "socks-wss",
        "tls-failure",
    ):
        context = tls["h2" if mode == "direct" else "http/1.1"]
        if mode == "tls-failure":
            context.minimum_version = ssl.TLSVersion.TLSv1_3
        proxy = mode in ("connect", "secure-connect", "connect-auth", "socks-wss")
        with ProxyPeer(mode, context) if proxy else DirectPeer(mode, context) as peer:
            serving = threading.Thread(
                target=peer.serve_forever, kwargs={"poll_interval": 0.01}, daemon=True
            )
            serving.start()
            try:
                result = subprocess.run(
                    [binary, mode, str(peer.server_address[1])],
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                assert result.returncode == 0, (mode, result.stdout, result.stderr, peer.errors)
                assert result.stdout == f"secure observer case passed: {mode}\n"
                assert not result.stderr and not peer.errors, (mode, result.stderr, peer.errors)
                assert peer.connections == (2 if mode == "connect-auth" else 1)
                if proxy:
                    assert peer.upgrades == 1
                elif mode == "tls-failure":
                    assert peer.failures == 1 and peer.requests == 0
                else:
                    assert peer.requests == (2 if mode == "direct" else 1)
            finally:
                peer.shutdown()
                serving.join()
    print("client observer DNS TLS proxy TLS tunnel and HTTP2 reuse timing passed")


if __name__ == "__main__":
    main()
