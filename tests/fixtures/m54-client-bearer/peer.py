import contextlib
import os
import socket
import subprocess
import sys
import threading

import http1_peer


def consume(binary, mode, *urls, moving=True):
    environment = dict(os.environ)
    if moving:
        environment.setdefault("SCOOP_GC_STRESS_MOVE", "0")
    else:
        environment.pop("SCOOP_GC_STRESS_MOVE", None)
    result = subprocess.run(
        [binary, mode, *urls],
        env=environment,
        capture_output=True,
        text=True,
        timeout=600 if mode in ("cache", "http2") else 180,
    )
    assert result.returncode == 0, (mode, urls, result.returncode, result.stdout, result.stderr)
    assert result.stdout == f"client Bearer {mode}\n" and not result.stderr, result


def exercise(binary, mode, serve, moving=True):
    failures = []
    with contextlib.ExitStack() as stack:
        listeners = [stack.enter_context(socket.socket()) for _ in range(4)]
        for listener in listeners:
            listener.bind(("127.0.0.1", 0))
            listener.listen(4)
            listener.settimeout(45)

        def worker(first, second):
            try:
                serve(first, second)
            except BaseException as error:
                failures.append(error)

        workers = [
            threading.Thread(target=worker, args=pair, daemon=True)
            for pair in (listeners[:2], listeners[2:])
        ]
        for thread in workers:
            thread.start()
        consume(
            binary,
            mode,
            *[f"http://127.0.0.1:{s.getsockname()[1]}/private/start" for s in listeners],
            moving=moving,
        )
        for thread in workers:
            thread.join(45)
            assert not thread.is_alive(), (mode, "authentication peer did not finish")
        assert not failures, (mode, failures)
        for listener in listeners:
            listener.settimeout(0.05)
            try:
                extra, _ = listener.accept()
            except TimeoutError:
                continue
            extra.close()
            raise AssertionError((mode, "unexpected additional connection"))


def main():
    for mode in ("cache", "refresh", "same", "self", "http2"):
        consume(sys.argv[1], mode)
    for case in ("caller", "total"):
        consume(sys.argv[1], "shared", case)
    for case in ("caller", "total", "clear", "client"):
        for cleanup in ("none", "plain", "wrapped"):
            consume(sys.argv[1], "cancel", case, cleanup)
    for case in (
        "challenge",
        "realm",
        "preemptive",
        "realm-miss",
        "token68",
        "malformed",
        "scope",
        "request",
        "unknown",
        "forbidden",
        "manual",
        "disabled",
        "zero",
        "limit",
        "sent",
        "unoffered",
        "origin",
    ):
        consume(sys.argv[1], "policy", case)
    for case in (
        "empty",
        "space",
        "unicode",
        "control",
        "padding",
        "load-callback",
        "load-nested",
        "refresh-none",
        "refresh-same",
        "refresh-bad",
        "refresh-callback",
        "refresh-nested",
    ):
        consume(sys.argv[1], "credentials", case)
    exercise(sys.argv[1], "challenge", http1_peer.challenge)
    exercise(sys.argv[1], "redirect", http1_peer.redirect)
    print("independent Bearer cache refresh cancellation and HTTP2 passed")


if __name__ == "__main__":
    main()
