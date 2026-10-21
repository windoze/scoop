import base64
import contextlib
import ipaddress
import os
import socket
import subprocess
import sys
import threading
import urllib.parse
from pathlib import Path

import tls_peer
import wire

MODES = (
    "default",
    "http",
    "lower-priority",
    "cgi",
    "no-proxy-ip",
    "no-proxy-domain",
    "no-proxy-port",
    "no-proxy-all",
    "override",
    "redirect",
    "redirect-override-301",
    "redirect-override-302",
    "redirect-override-303",
    "socks-local",
    "socks-remote",
    "https-proxy",
    "invalid-utf8",
)


def environment(mode, proxy, origin_port):
    values = dict(os.environb)
    for name in (b"http_proxy", b"https_proxy", b"all_proxy", b"no_proxy"):
        values.pop(name, None)
        values.pop(name.upper(), None)
    values.pop(b"REQUEST_METHOD", None)
    values[b"ALL_PROXY"] = proxy.encode()
    if mode in ("http", "lower-priority", "https-proxy"):
        values[b"http_proxy"] = proxy.encode()
        values[b"HTTP_PROXY"] = b"\xff"
    if mode == "cgi":
        values.pop(b"ALL_PROXY")
        values[b"REQUEST_METHOD"] = b"\xff"
        values[b"HTTP_PROXY"] = b"\xff"
    if mode in ("no-proxy-ip", "redirect"):
        values[b"NO_PROXY"] = b"127.0.0.0/8"
    if mode == "no-proxy-domain":
        values[b"NO_PROXY"] = b"LOCALHOST."
    if mode == "no-proxy-port":
        values[b"NO_PROXY"] = f"127.0.0.0/8:{origin_port}".encode()
    if mode == "no-proxy-all":
        values[b"NO_PROXY"] = b"*"
    if mode == "invalid-utf8":
        values[b"http_proxy"] = b"\xff"
    return values


def socks(connection, remote, target):
    assert wire.exact(connection, 3) == b"\x05\x01\x02"
    connection.sendall(b"\x05\x02")
    assert wire.exact(connection, 1) == b"\x01"
    username = wire.exact(connection, wire.exact(connection, 1)[0])
    password = wire.exact(connection, wire.exact(connection, 1)[0])
    assert username == "Jose\u0301".encode() and password == b"p:ss"
    connection.sendall(b"\x01\0")
    version, command, reserved, kind = wire.exact(connection, 4)
    assert (version, command, reserved) == (5, 1, 0)
    count = wire.exact(connection, 1)[0] if kind == 3 else {1: 4, 4: 16}[kind]
    address = wire.exact(connection, count)
    assert int.from_bytes(wire.exact(connection, 2), "big") == target.port
    if remote:
        assert kind == 3 and address == target.hostname.encode()
    else:
        assert kind in (1, 4) and ipaddress.ip_address(address).is_loopback
    connection.sendall(b"\x05\0\0\x01\x7f\0\0\x01\0\0")


def exercise(binary, mode, contexts):
    failures = []
    with contextlib.ExitStack() as stack:
        proxy = stack.enter_context(socket.socket())
        origin = stack.enter_context(socket.socket())
        for listener in (proxy, origin):
            listener.bind(("127.0.0.1", 0))
            listener.listen(4)
            listener.settimeout(180)
        proxy_port, origin_port = proxy.getsockname()[1], origin.getsockname()[1]
        rewriting = mode.startswith("redirect-override-")
        direct = rewriting or mode in (
            "default",
            "cgi",
            "no-proxy-ip",
            "no-proxy-domain",
            "no-proxy-port",
            "no-proxy-all",
            "override",
            "redirect",
        )
        host = (
            "localhost"
            if mode in ("no-proxy-domain", "socks-local")
            else "127.0.0.1"
            if direct
            else "origin.invalid"
        )
        target = f"http://{host}:{origin_port}/a%2Fb?q=%2F#fragment"
        final = f"http://origin.invalid:{origin_port}/done" if mode == "redirect" else target
        scheme = {"socks-local": "socks5", "socks-remote": "socks5h", "https-proxy": "https"}.get(
            mode, "http"
        )
        proxy_url = f"{scheme}://Jose%CC%81:p%3Ass@127.0.0.1:{proxy_port}"

        def serve():
            try:
                if direct:
                    with origin.accept()[0] as connection:
                        first, headers = wire.request(connection)
                        method = "POST" if rewriting else "GET"
                        assert first == f"{method} /a%2Fb?q=%2F HTTP/1.1".encode(), first
                        assert (
                            b"authorization" not in headers
                            and b"proxy-authorization" not in headers
                        )
                        if rewriting:
                            status = mode.rsplit("-", 1)[1]
                            connection.sendall(
                                f"HTTP/1.1 {status} Redirect\r\nLocation: /done\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".encode()
                            )
                        elif mode == "redirect":
                            connection.sendall(
                                b"HTTP/1.1 302 Redirect\r\nLocation: "
                                + final.encode()
                                + b"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            )
                        else:
                            connection.sendall(
                                b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nenvironment"
                            )
                            return
                    if rewriting:
                        with origin.accept()[0] as connection:
                            first, headers = wire.request(connection)
                            assert first == b"GET /done HTTP/1.1", first
                            assert b"proxy-authorization" not in headers, headers
                            connection.sendall(
                                b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nenvironment"
                            )
                        return
                with proxy.accept()[0] as transport:
                    connection = (
                        tls_peer.TlsLayer(transport, contexts["http/1.1"])
                        if mode == "https-proxy"
                        else transport
                    )
                    parsed = urllib.parse.urlsplit(final)
                    is_socks = mode.startswith("socks-")
                    if is_socks:
                        socks(connection, mode == "socks-remote", parsed)
                    first, headers = wire.request(connection)
                    path = parsed.path + ("?" + parsed.query if parsed.query else "")
                    expected = (
                        path if is_socks else urllib.parse.urlunsplit(parsed._replace(fragment=""))
                    )
                    assert first == f"GET {expected} HTTP/1.1".encode(), first
                    assert b"authorization" not in headers, headers
                    if is_socks:
                        assert b"proxy-authorization" not in headers
                    else:
                        scheme, token = headers[b"proxy-authorization"].split(b" ", 1)
                        assert scheme.lower() == b"basic", headers
                        assert token == base64.b64encode("José:p:ss".encode()), headers
                    connection.sendall(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nenvironment"
                    )
            except BaseException as error:
                failures.append(error)

        worker = threading.Thread(target=serve, daemon=True)
        if mode != "invalid-utf8":
            worker.start()
        result = subprocess.run(
            [binary, mode, target],
            env=environment(mode, proxy_url, origin_port),
            capture_output=True,
            text=True,
            timeout=300,
        )
        assert result.returncode == 0, (
            mode,
            result.returncode,
            failures,
            result.stdout,
            result.stderr,
        )
        assert result.stdout == f"environment proxy {mode}\n" and not result.stderr, result
        if mode != "invalid-utf8":
            worker.join(180)
            assert not worker.is_alive(), (mode, "environment proxy peer did not finish")
        assert not failures, (mode, failures)
        for listener in (proxy, origin):
            listener.settimeout(0.02)
            try:
                extra, _ = listener.accept()
            except TimeoutError:
                continue
            extra.close()
            raise AssertionError((mode, "unexpected route connection"))


def main():
    binary, server, intermediate, output = sys.argv[1:]
    contexts = tls_peer.contexts(server, intermediate, Path(output))
    for mode in MODES:
        exercise(binary, mode, contexts)
    print("independent environment proxy HTTP TLS SOCKS bypass and redirect routes passed")


if __name__ == "__main__":
    main()
