import contextlib
import json
import os
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

import http_peer
import socks_peer
import tls_peer
import wire

MODES = (
    "forward",
    "forward-tls",
    "https",
    "https-proxy",
    "h2",
    "h2-proxy",
    "h2-clear",
    "h2-upgrade",
    "http10",
    "socks-domain",
    "socks-local",
    "socks-ipv6",
    "socks-auth",
    "socks-h2",
    "socks-tls",
    "reject",
    "socks-reject",
    "proxy-zero",
    "socks-zero",
    "proxy-deadline",
    "socks-deadline",
    "target-untrusted",
    "proxy-untrusted",
    "reuse",
    "routes",
    "identity",
    "override",
    "digest",
    "retry",
    "redirect",
)


def consume(binary, mode, arguments):
    result = subprocess.run(
        [binary, mode, *arguments],
        capture_output=True,
        text=True,
        env=dict(os.environ, SCOOP_GC_STRESS_MOVE="1"),
        timeout=600,
    )
    assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
    assert result.stdout == f"client proxy {mode}\n" and not result.stderr, result


def exercise(binary, mode, contexts):
    outer_tls = mode in (
        "forward-tls",
        "https-proxy",
        "h2-proxy",
        "target-untrusted",
        "proxy-untrusted",
    )
    target_tls = mode in (
        "https",
        "https-proxy",
        "h2",
        "h2-proxy",
        "socks-tls",
        "reject",
        "proxy-zero",
        "proxy-deadline",
        "target-untrusted",
        "proxy-untrusted",
    )
    failures = []
    with contextlib.ExitStack() as stack:
        listeners = [stack.enter_context(socket.socket()) for _ in range(6)]
        for listener in listeners:
            listener.bind(("127.0.0.1", 0))
            listener.listen(4)
            listener.settimeout(180)
        arguments = []
        targets = []
        for group in (listeners[:3], listeners[3:]):
            scheme = "socks5" if mode.startswith("socks-") else "https" if outer_tls else "http"
            arguments.extend(
                f"{scheme}://127.0.0.1:{listener.getsockname()[1]}" for listener in group[:2]
            )
            host = (
                "localhost"
                if target_tls or mode == "socks-local"
                else "[::1]"
                if mode == "socks-ipv6"
                else "127.0.0.1"
                if mode == "override"
                else "origin.invalid"
            )
            target = f"{'https' if target_tls else 'http'}://user:unused@{host}:{group[2].getsockname()[1]}/a%2Fb?q=x%20y#fragment"
            arguments.append(target)
            targets.append(target)

        def worker(group, target):
            try:
                if mode in ("reuse", "routes", "identity", "override"):
                    http_peer.routing(group, mode, target)
                    return
                if mode == "digest":
                    http_peer.digest(group[0], target)
                    return
                if mode in ("retry", "redirect"):
                    http_peer.follow_up(group, mode, target)
                    return
                network_mode = mode
                if mode in ("proxy-deadline", "socks-deadline"):
                    with group[0].accept()[0] as stalled:
                        stalled.settimeout(180)
                        if mode == "socks-deadline":
                            socks_peer.serve(stalled, mode, target, contexts)
                            network_mode = "socks-domain"
                        else:
                            http_peer.tunnel(stalled, mode, target, contexts)
                            network_mode = "https"
                with group[0].accept()[0] as raw:
                    raw.settimeout(180)
                    if mode in ("proxy-zero", "socks-zero"):
                        wire.closed(raw)
                        return
                    connection = raw
                    if mode == "proxy-untrusted":
                        tls_peer.untrusted(connection, contexts["http/1.1"])
                        return
                    if outer_tls:
                        connection = tls_peer.TlsLayer(connection, contexts["http/1.1"])
                        assert connection.ssl.selected_alpn_protocol() == "http/1.1"
                    if mode.startswith("socks-"):
                        socks_peer.serve(connection, network_mode, target, contexts)
                    elif mode in ("forward", "forward-tls"):
                        http_peer.request(connection, target, forward=True, method=b"POST")
                        http_peer.response(connection)
                        wire.closed(connection)
                    else:
                        http_peer.tunnel(connection, network_mode, target, contexts)
            except BaseException as error:
                failures.append(error)

        workers = [
            threading.Thread(target=worker, args=(group, target), daemon=True)
            for group, target in zip((listeners[:3], listeners[3:]), targets, strict=True)
        ]
        for thread in workers:
            thread.start()
        try:
            consume(binary, mode, arguments)
        except Exception as error:
            if failures:
                raise AssertionError((mode, failures)) from error
            raise
        for thread in workers:
            thread.join(180)
            assert not thread.is_alive(), (mode, "proxy peer did not finish")
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
    consume(binary, "options", [])
    timings = []
    for mode in MODES:
        start = time.monotonic()
        exercise(binary, mode, contexts)
        timings.append({"mode": mode, "seconds": round(time.monotonic() - start, 3)})
        (directory / "timings.json").write_text(json.dumps(timings, indent=2))
    print("independent HTTP HTTPS CONNECT SOCKS5 routing and Digest passed")


if __name__ == "__main__":
    main()
