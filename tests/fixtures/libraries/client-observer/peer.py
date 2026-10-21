import subprocess
import sys
import threading

from peer_server import Peer

COUNTS = {
    "h1": 2,
    "off": 1,
    "follow-up": 4,
    "cache": 2,
    "dns": 1,
    "tcp-callback": 1,
    "partial-head": 1,
    "partial-body": 1,
    "websocket": 1,
    "websocket-rejected": 1,
    "websocket-failure": 2,
    "websocket-total": 2,
    "sse": 1,
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
                assert result.returncode == 0, (
                    mode,
                    result.stdout,
                    result.stderr,
                    peer.errors,
                    peer.requests,
                )
                assert result.stdout == f"client observer case passed: {mode}\n"
                assert not result.stderr, result.stderr
                assert not peer.errors, (mode, peer.errors)
                assert len(peer.requests) == count, (mode, peer.requests)
                if mode == "h1":
                    assert peer.connections == 1
            finally:
                peer.shutdown()
                serving.join()
    print("client observer timing progress retries cache context streams and redaction passed")


if __name__ == "__main__":
    main()
