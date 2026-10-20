import contextlib
import os
import socket
import subprocess
import sys
import threading

from http1_peer import serve


def exercise(binary, mode):
    failures = []
    with contextlib.ExitStack() as stack:
        listeners = [stack.enter_context(socket.socket()) for _ in range(2)]
        for listener in listeners:
            listener.bind(("127.0.0.1", 0))
            listener.listen(4)
            listener.settimeout(60)

        def worker(listener):
            try:
                serve(listener, mode)
            except BaseException as error:
                failures.append(error)

        workers = [
            threading.Thread(target=worker, args=(listener,), daemon=True) for listener in listeners
        ]
        for worker in workers:
            worker.start()
        result = subprocess.run(
            [
                binary,
                mode,
                *[f"http://127.0.0.1:{s.getsockname()[1]}/private/start" for s in listeners],
            ],
            env=os.environ,
            capture_output=True,
            text=True,
            timeout=180,
        )
        for worker in workers:
            worker.join(60)
            assert not worker.is_alive(), (mode, "Digest peer did not finish")
        assert not failures, (mode, failures, result)
        assert (
            result.returncode == 0
            and result.stdout == f"client Digest {mode}\n"
            and not result.stderr
        ), result
        for listener in listeners:
            listener.settimeout(0.05)
            try:
                extra, _ = listener.accept()
            except TimeoutError:
                continue
            extra.close()
            raise AssertionError((mode, "unexpected additional Digest connection"))


def main():
    modes = [
        f"{algorithm}_{qop}"
        for algorithm in (
            "MD5",
            "MD5-sess",
            "SHA-256",
            "SHA-256-sess",
            "SHA-512-256",
            "SHA-512-256-sess",
        )
        for qop in ("auth", "auth-int", "legacy")
    ]
    modes += [
        "default-md5",
        "utf8",
        "userhash",
        "latin",
        "preference",
        "integrity-preference",
        "auth-preference",
        "gzip",
        "trailer",
        "provided",
        "provided-sent",
        "nohash",
        "stale",
        "rejected",
        "no-stale",
        "limit",
        "cache",
        "nextnonce",
        "preempt-rejected",
        "clear",
        "no-info",
        "invalid-token",
        "invalid-nonce",
        "invalid-realm",
        "invalid-syntax",
        "unsupported-algorithm",
        "unsupported-qop",
        "unsupported-charset",
        "unsupported-realm",
        "bad-proof",
        "bad-hex",
        "bad-qop",
        "bad-nc",
        "bad-cnonce",
        "bad-nextnonce",
        "bad-missing",
        "bad-syntax",
        "bad-body",
        "bad-trailer-duplicate",
    ]
    for mode in modes:
        exercise(sys.argv[1], mode)
    print("independent Digest algorithms bodies nonce lifecycle and response proofs passed")


if __name__ == "__main__":
    main()
