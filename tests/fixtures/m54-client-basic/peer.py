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
        environment["SCOOP_GC_STRESS_MOVE"] = "1"
    else:
        environment.pop("SCOOP_GC_STRESS_MOVE", None)
    result = subprocess.run(
        [binary, mode, *urls],
        env=environment,
        capture_output=True,
        text=True,
        timeout=180,
    )
    assert result.returncode == 0, (mode, urls, result.returncode, result.stdout, result.stderr)
    assert result.stdout == f"client Basic {mode}\n" and not result.stderr, result


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
    for group, cases in (
        (
            "policy",
            ("manual", "disabled", "realm", "missing", "malformed", "sent", "rejected", "limit"),
        ),
        (
            "credentials",
            ("preemptive", "zero", "user", "password", "encoding", "callback", "nested"),
        ),
        ("cancel", ("caller", "total", "client")),
        ("cleanup", ("success", "failure")),
    ):
        for case in cases:
            consume(sys.argv[1], group, case)
    consume(sys.argv[1], "selection")
    consume(sys.argv[1], "http2")
    exercise(sys.argv[1], "challenge", http1_peer.challenge)
    exercise(sys.argv[1], "redirect", http1_peer.redirect)
    print("independent Basic credentials redirects cancellation and HTTP2 passed")


if __name__ == "__main__":
    main()
