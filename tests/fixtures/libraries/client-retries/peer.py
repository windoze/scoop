import contextlib
import os
import socket
import subprocess
import sys
import threading

import http1_peer
import http2_peer


def exercise(binary, mode, serve):
    failures = []
    with contextlib.ExitStack() as stack:
        listeners = [stack.enter_context(socket.socket()) for _ in range(2)]
        for listener in listeners:
            listener.bind(("127.0.0.1", 0))
            listener.listen(4)
            listener.settimeout(45)

        def worker(listener):
            try:
                serve(listener)
            except BaseException as error:
                failures.append(error)

        workers = [
            threading.Thread(target=worker, args=(listener,), daemon=True) for listener in listeners
        ]
        for thread in workers:
            thread.start()
        result = subprocess.run(
            [binary, mode, *[f"http://127.0.0.1:{s.getsockname()[1]}/start" for s in listeners]],
            env=os.environ,
            capture_output=True,
            text=True,
            timeout=180,
        )
        for thread in workers:
            thread.join(45)
            assert not thread.is_alive(), (mode, "retry peer did not finish")
        assert result.returncode == 0, (
            mode,
            result.returncode,
            result.stdout,
            result.stderr,
            failures,
        )
        assert not failures, (mode, failures)
        assert result.stdout == f"client retries {mode}\n" and not result.stderr
        for listener in listeners:
            listener.settimeout(0.05)
            try:
                extra, _ = listener.accept()
            except TimeoutError:
                continue
            extra.close()
            raise AssertionError((mode, "unexpected additional connection"))


for case, handler in (
    ("replay", http1_peer.replay),
    ("refused", http2_peer.refused),
    ("goaway", http2_peer.goaway),
    ("status", http1_peer.status),
    ("post", http1_peer.post),
    ("total", http1_peer.total),
):
    exercise(sys.argv[1], case, handler)
print("independent HTTP retries replay refusal redirects and total deadline passed")
