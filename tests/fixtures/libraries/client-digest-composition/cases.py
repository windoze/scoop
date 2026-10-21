import contextlib
import os
import socket
import subprocess
import sys
import threading

from concurrent_peer import clear_active, concurrent
from http1_peer import serve
from routing_peer import no_preempt, retry, routing


def consume(mode, *arguments):
    result = subprocess.run(
        [sys.argv[1], mode, *arguments],
        env=os.environ,
        capture_output=True,
        text=True,
        timeout=600 if mode in ("http2", "late-401") else 180,
    )
    assert (
        result.returncode == 0
        and result.stdout == f"Digest composition {mode}\n"
        and not result.stderr
    ), result


def external(mode, server):
    failures = []
    with contextlib.ExitStack() as stack:
        listeners = [stack.enter_context(socket.socket()) for _ in range(2)]
        for listener in listeners:
            listener.bind(("127.0.0.1", 0))
            listener.listen(4)
            listener.settimeout(60)

        def run(listener):
            try:
                server(listener)
            except BaseException as error:
                failures.append(error)

        threads = [
            threading.Thread(target=run, args=(listener,), daemon=True) for listener in listeners
        ]
        for thread in threads:
            thread.start()
        try:
            consume(
                mode,
                *[
                    f"http://127.0.0.1:{listener.getsockname()[1]}/private/start"
                    for listener in listeners
                ],
            )
        finally:
            for thread in threads:
                thread.join(60)
            assert not failures, (mode, failures)
            assert all(not thread.is_alive() for thread in threads), mode


for case in ("callback", "nested", "user", "password", "encoding"):
    consume("credentials", case)
consume("http2")
external("metadata", lambda listener: serve(listener, "provided"))
for case in ("routing", "domain"):
    external(case, lambda listener, mode=case: routing(listener, mode))
external("retry", retry)
external("no-preempt", no_preempt)
for case in ("late-401", "late-nextnonce"):
    external(case, lambda listener, mode=case: concurrent(listener, mode))
external("clear-active", clear_active)
print("Digest metadata redirects retries HTTP2 and concurrent nonce updates passed")
