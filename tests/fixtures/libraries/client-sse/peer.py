import http.server
import os
import subprocess
import sys
import threading
from pathlib import Path

from peer_server import COUNTS, FAILURES, Peer
from tls_peer import contexts
from transport_peer import exercise_transport

MODES = (
    "plain",
    "gzip",
    "post",
    "post-once",
    "header",
    "cache",
    "stop",
    "reconnect",
    "post-reconnect",
    "cookie",
    "limit",
    "truncated",
    "bad-chunk",
    "bad-gzip",
    "line-limit",
    "missing",
    "media",
    "duplicate",
    "bad-media",
    "status",
    "other-success",
    "callback-error",
    "callback-close",
    "informational-error",
    "redirect",
)
TIMEOUT = 600 if os.environ.get("SCOOP_GC_STRESS_MOVE") == "1" else 180
with http.server.ThreadingHTTPServer(("127.0.0.1", 0), Peer) as server:
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    try:
        for mode in MODES:
            result = subprocess.run(
                [sys.argv[1], mode, f"http://127.0.0.1:{server.server_port}/{mode}"],
                capture_output=True,
                text=True,
                timeout=TIMEOUT,
                env=os.environ,
            )
            assert not FAILURES, (mode, FAILURES, result.stdout, result.stderr)
            assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
            assert result.stdout == f"client SSE {mode}\n" and not result.stderr
            expected = (
                3
                if mode in ("reconnect", "post-reconnect", "cookie")
                else 2
                if mode
                in (
                    "cache",
                    "limit",
                    "truncated",
                    "callback-close",
                    "informational-error",
                    "redirect",
                )
                else 1
            )
            assert COUNTS[mode] == expected, (mode, COUNTS)
            assert COUNTS[mode + "/probe"] == 1, (mode, COUNTS)
        assert not FAILURES, FAILURES
    finally:
        server.shutdown()
        worker.join(TIMEOUT)
tls = contexts(sys.argv[2], sys.argv[3], Path(sys.argv[4]))
for mode in ("tls-eof", "h2-eof", "pool-eof", "tls-h2-eof", "h2-protocol", "refused"):
    repeats = 16 if mode == "h2-eof" else 8 if mode == "pool-eof" else 1
    for _ in range(repeats):
        exercise_transport(
            sys.argv[1], mode, tls["http/1.1" if mode == "tls-eof" else "h2"], TIMEOUT
        )
print("client SSE independent framing replay cursors reconnection and failures passed")
