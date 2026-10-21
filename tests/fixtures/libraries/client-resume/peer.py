import http.server
import os
import subprocess
import sys
import threading
from pathlib import Path

from peer_server import COUNTS, ENCODED, FAILURES, PAYLOAD, REPLACEMENT, TRACES, Peer

MODES = (
    "resume",
    "segments",
    "unknown-size",
    "raw",
    "restart",
    "mismatch",
    "fail-200",
    "fail-tag",
    "start",
    "total",
    "encoding",
    "unknown-range",
    "length",
    "short",
    "long",
    "truncated-range",
    "complete-416",
    "bad-416",
    "cookie",
    "authorization",
    "final-url",
    "status",
    "cache",
    "digest-incomplete",
    "digest-prefix",
)
SUCCESS = {
    "resume",
    "segments",
    "unknown-size",
    "raw",
    "restart",
    "mismatch",
    "complete-416",
    "cache",
    "digest-incomplete",
    "digest-prefix",
}
TIMEOUT = 600 if os.environ.get("SCOOP_GC_STRESS_MOVE") == "1" else 180
binary, root = sys.argv[1], Path(sys.argv[2])
root.mkdir(parents=True, exist_ok=True)
with http.server.ThreadingHTTPServer(("127.0.0.1", 0), Peer) as server:
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        for mode in MODES:
            directory = root / mode
            directory.mkdir()
            result = subprocess.run(
                [binary, mode, f"http://127.0.0.1:{server.server_port}/{mode}", str(directory)],
                capture_output=True,
                text=True,
                timeout=TIMEOUT,
                env=os.environ,
            )
            assert not FAILURES, (mode, FAILURES, result.stdout, result.stderr)
            assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
            assert result.stdout == f"client resume {mode}\n" and not result.stderr
            expected = (
                ENCODED
                if mode == "raw"
                else REPLACEMENT
                if mode in ("restart", "mismatch")
                else PAYLOAD
            )
            if mode in SUCCESS:
                assert (directory / "result").read_bytes() == expected, mode
                assert sorted(p.name for p in directory.iterdir()) == ["result"], mode
            else:
                assert (directory / "result").read_bytes() == b"original", mode
                size = (
                    45
                    if mode == "short"
                    else 48
                    if mode in ("long", "bad-416")
                    else 28
                    if mode == "truncated-range"
                    else 8
                )
                assert (directory / "partial").read_bytes() == PAYLOAD[:size], mode
                assert b"password" not in (directory / "metadata").read_bytes(), mode
                assert sorted(p.name for p in directory.iterdir()) == [
                    "metadata",
                    "partial",
                    "result",
                ], mode
            assert COUNTS[mode] == (
                3 if mode in ("segments", "mismatch", "final-url", "cache", "digest-prefix") else 2
            ), (mode, COUNTS, TRACES)
        assert not FAILURES, FAILURES
    finally:
        server.shutdown()
        worker.join(TIMEOUT)
print("client resume independent ranges identity integrity and file commit passed")
