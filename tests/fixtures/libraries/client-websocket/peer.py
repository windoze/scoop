import subprocess
import sys
import threading

from peer_server import Peer

MODES = (
    "plain",
    "force-h1",
    "force-h2",
    "reject",
    "bad-accept",
    "bad-protocol",
    "bad-extensions",
    "redirect",
    "ws-redirect",
    "retry",
    "cookie",
    "deflate",
    "bad-window",
)


def main():
    for mode in MODES:
        with Peer(mode) as peer:
            serving = threading.Thread(
                target=peer.serve_forever, kwargs={"poll_interval": 0.01}, daemon=True
            )
            serving.start()
            try:
                result = subprocess.run(
                    [sys.argv[1], mode, str(peer.server_address[1])],
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                assert result.returncode == 0, (mode, result.stdout, result.stderr, peer.errors)
                assert result.stdout == f"client WebSocket case passed: {mode}\n", result.stdout
                assert not result.stderr, result.stderr
                assert not peer.errors, (mode, peer.errors)
                assert len(peer.keys) == (2 if mode in ("redirect", "ws-redirect", "retry") else 1)
                assert peer.probes == (0 if mode == "force-h2" else 1)
            finally:
                peer.shutdown()
                serving.join()
    print("client WebSocket independent handshake routing messages and rejections passed")


if __name__ == "__main__":
    main()
