import subprocess
import sys
import threading
from pathlib import Path

from peer_server import ProxyPeer
from tls_peer import contexts


def main():
    binary, certificate_source, directory = sys.argv[1:]
    source = Path(certificate_source)
    context = contexts(source / "server.scoop", source / "intermediate.scoop", Path(directory))[
        "http/1.1"
    ]
    for mode in (
        "connect",
        "secure-connect",
        "connect-auth",
        "socks-proxy",
        "socks-local",
        "socks-auth",
        "socks-wss",
    ):
        with ProxyPeer(mode, context) as peer:
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
                assert result.stdout == f"tunneled client WebSocket case passed: {mode}\n"
                assert not result.stderr, result.stderr
                assert not peer.errors, (mode, peer.errors)
                assert peer.connections == (2 if mode == "connect-auth" else 1)
                assert peer.upgrades == 1
            finally:
                peer.shutdown()
                serving.join()
    print("client WebSocket CONNECT TLS-in-TLS and SOCKS5 routing passed")


if __name__ == "__main__":
    main()
