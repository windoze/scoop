import subprocess
import sys
import threading
from pathlib import Path

from peer_server import AuthPeer
from tls_peer import contexts


def main():
    binary, certificate_source, directory = sys.argv[1:]
    source = Path(certificate_source)
    secure = contexts(source / "server.scoop", source / "intermediate.scoop", Path(directory))[
        "http/1.1"
    ]
    modes = (
        "basic",
        "bearer",
        "refresh",
        "digest",
        "integrity",
        "bad-proof",
        "callback",
        "proxy-basic",
        "proxy-bearer",
        "proxy-digest",
        "secure-proxy-digest",
    )
    for mode in modes:
        with AuthPeer(mode, secure if mode == "secure-proxy-digest" else None) as peer:
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
                assert result.stdout == f"authenticated client WebSocket case passed: {mode}\n"
                assert not result.stderr, result.stderr
                assert not peer.errors, (mode, peer.errors)
                assert len(peer.keys) == (1 if mode == "callback" else 2)
            finally:
                peer.shutdown()
                serving.join()
    print("client WebSocket Basic Bearer Digest and HTTP HTTPS proxy authentication passed")


if __name__ == "__main__":
    main()
