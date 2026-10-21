import subprocess
import sys
import threading

from peer_server import Peer

COUNTS = {
    "chain": 4,
    "once": 2,
    "short": 2,
    "replace": 3,
    "throw": 2,
    "owner": 2,
    "kind": 2,
    "attempt-throw": 1,
    "fields": 1,
    "cache": 3,
    "cache-refresh": 3,
    "proxy": 2,
    "auth": 1,
    "websocket": 2,
    "websocket-invalid": 1,
}


def main():
    for mode, count in COUNTS.items():
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
                assert result.stdout == f"client interceptor case passed: {mode}\n", result.stdout
                assert not result.stderr, result.stderr
                assert not peer.errors, (mode, peer.errors)
                assert len(peer.requests) == count, (mode, peer.requests)
            finally:
                peer.shutdown()
                serving.join()
    print("client interceptors signatures cache identity WebSocket and resource ownership passed")


if __name__ == "__main__":
    main()
