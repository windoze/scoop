import socketserver
import ssl
import subprocess
import sys
import threading
import traceback
from pathlib import Path

import h2_wire
from network_support import pem_function
from peer_server import Handler


class SecureHandler(Handler):
    def request_loop(self):
        if self.connection.selected_alpn_protocol() == "h2":
            assert self.server.coexist
            h2_wire.begin(self.connection)
            h2_wire.h2_request(self.connection, 1)
            self.connection.sendall(
                h2_wire.encoded(1, 4, 1, b"\x88") + h2_wire.encoded(0, 0, 1, b"hold")
            )
            h2_wire.h2_request(self.connection, 3)
            self.connection.sendall(
                h2_wire.encoded(1, 4, 3, b"\x88") + h2_wire.encoded(0, 1, 3, b"probe")
            )
            self.server.probes += 1
            assert self.server.websocket_closed.wait(30), "WebSocket did not close"
            self.connection.sendall(h2_wire.encoded(0, 1, 1, b" done"))
            h2_wire.h2_request(self.connection, 5)
            self.connection.sendall(
                h2_wire.encoded(1, 4, 5, b"\x88") + h2_wire.encoded(0, 1, 5, b"probe")
            )
            self.server.probes += 1
            h2_wire.closed(self.connection, h2=True)
        else:
            super().request_loop()
            self.server.websocket_closed.set()


class SecurePeer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, mode, context):
        super().__init__(("127.0.0.1", 0), SecureHandler)
        self.context = context
        self.mode = "deflate" if mode == "deflate" else "plain"
        self.coexist = mode in ("coexist", "mutual")
        self.mutual = mode == "mutual"
        self.errors = []
        self.keys = []
        self.probes = 0
        self.protocols = []
        self.lock = threading.Lock()
        self.websocket_closed = threading.Event()

    def get_request(self):
        raw, address = super().get_request()
        try:
            raw.settimeout(30)
            secure = self.context.wrap_socket(raw, server_side=True)
            self.protocols.append(secure.selected_alpn_protocol())
            assert bool(secure.getpeercert()) == self.mutual
            return secure, address
        except Exception:
            self.errors.append(traceback.format_exc())
            raw.close()
            raise


def main():
    binary, source, directory = sys.argv[1:]
    source, directory = Path(source), Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    chain, key, root = (directory / name for name in ("chain.pem", "key.pem", "root.pem"))
    chain.write_text(
        pem_function(source / "server.scoop", "serverPem")
        + pem_function(source / "intermediate.scoop", "intermediatePem")
    )
    key.write_text(pem_function(source / "server.scoop", "serverKeyPem"))
    root.write_text(pem_function(source / "root.scoop", "rootPem"))
    for mode in ("coexist", "h2-only", "no-alpn", "mutual", "deflate"):
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(chain, key)
        context.set_alpn_protocols(
            []
            if mode == "no-alpn"
            else ["h2", "http/1.1"]
            if mode in ("coexist", "mutual")
            else ["http/1.1"]
        )
        if mode == "mutual":
            context.verify_mode = ssl.CERT_REQUIRED
            context.load_verify_locations(root)
        with SecurePeer(mode, context) as peer:
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
                assert result.stdout == f"secure client WebSocket case passed: {mode}\n"
                assert not result.stderr, result.stderr
                assert not peer.errors, (mode, peer.errors)
                assert len(peer.keys) == 1
                expected = (
                    ["h2", "http/1.1"]
                    if peer.coexist
                    else ["http/1.1"]
                    if mode == "h2-only"
                    else [None, None]
                    if mode == "no-alpn"
                    else ["http/1.1", "http/1.1"]
                )
                assert peer.protocols == expected, (mode, peer.protocols)
                assert peer.probes == (2 if peer.coexist else 0 if mode == "h2-only" else 1)
            finally:
                peer.shutdown()
                serving.join()
    print("client WebSocket WSS H2 coexistence ALPN identity and compression passed")


if __name__ == "__main__":
    main()
