import contextlib
import json
import os
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

import digest_peer
import simple_peer
import tls_peer

MODES = (
    "basic-forward",
    "basic-connect",
    "basic-https-proxy",
    "basic-layered",
    "basic-rejected",
    "basic-limit",
    "basic-malformed",
    "basic-wrong-field",
    "basic-wrong-origin",
    "basic-static",
    "basic-sent",
    "basic-redirect",
    "bearer-forward",
    "bearer-refresh-connect",
    "bearer-cache-isolation",
    "nested-basic-forward",
    "nested-bearer-connect",
    "digest-forward",
    "digest-connect",
    "digest-gzip",
    "digest-nohash",
    "digest-stale",
    "digest-domain",
    "digest-double",
    "digest-bad-connect",
    "digest-bad-trailer",
    "digest-clear",
    "digest-h2-connect",
)


def exercise(binary, mode, contexts):
    failures = []
    with contextlib.ExitStack() as stack:
        listeners = [stack.enter_context(socket.socket()) for _ in range(6)]
        for listener in listeners:
            listener.bind(("127.0.0.1", 0))
            listener.listen(8)
            listener.settimeout(180)
        arguments = []
        groups = []
        for group in (listeners[:3], listeners[3:]):
            proxy = f"{'https' if mode == 'basic-https-proxy' else 'http'}://127.0.0.1:{group[0].getsockname()[1]}"
            second = f"http://127.0.0.1:{group[1].getsockname()[1]}"
            secure = mode.endswith("connect") or mode == "basic-https-proxy"
            target = f"{'https://localhost' if secure else 'http://origin.invalid'}:{group[2].getsockname()[1]}/a%2Fb?q=%2F#fragment"
            arguments.extend((proxy, second, target))
            groups.append((group, proxy, second, target))

        def serve(group, proxy, second, target):
            try:
                if mode == "bearer-cache-isolation":
                    simple_peer.isolation(group[0], proxy, contexts)
                elif mode == "digest-double":
                    digest_peer.double(group[0], proxy, contexts)
                elif mode.startswith("basic-"):
                    simple_peer.basic(group, mode, proxy, target, contexts)
                elif mode.startswith("nested-"):
                    simple_peer.nested(group, mode, second + "/token", target, contexts)
                elif mode.startswith("bearer-"):
                    simple_peer.bearer(group, mode, target, contexts)
                else:
                    digest_peer.digest(group[0], mode, target, contexts)
            except BaseException as error:
                failures.append(error)

        workers = [threading.Thread(target=serve, args=group, daemon=True) for group in groups]
        for worker in workers:
            worker.start()
        result = subprocess.run(
            [binary, mode, *arguments],
            capture_output=True,
            text=True,
            env=os.environ,
            timeout=600,
        )
        assert result.returncode == 0, (
            mode,
            result.returncode,
            failures,
            result.stdout,
            result.stderr,
        )
        assert result.stdout == f"proxy authentication {mode}\n" and not result.stderr, result
        for worker in workers:
            worker.join(180)
            assert not worker.is_alive(), (mode, "proxy authentication peer did not finish")
        assert not failures, (mode, failures)
        for listener in listeners:
            listener.settimeout(0.02)
            try:
                extra, _ = listener.accept()
            except TimeoutError:
                continue
            extra.close()
            raise AssertionError((mode, "unexpected additional connection"))


def main():
    binary, server_source, intermediate_source, output = sys.argv[1:]
    directory = Path(output)
    contexts = tls_peer.contexts(server_source, intermediate_source, directory)
    timings = []
    for mode in MODES:
        start = time.monotonic()
        exercise(binary, mode, contexts)
        timings.append({"mode": mode, "seconds": round(time.monotonic() - start, 3)})
        (directory / "timings.json").write_text(json.dumps(timings, indent=2))
    print("independent proxy Basic Bearer Digest and scoped authentication passed")


if __name__ == "__main__":
    main()
