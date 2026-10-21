import ast
import gzip
import os
import re
import select
import socket
import ssl
import subprocess
import sys
import threading
import traceback
from pathlib import Path

from wire import chunked, closed, exact, request, response, send_frame


def redirect(connection, status, location, length=0):
    connection.sendall(
        f"HTTP/1.1 {status} Move\r\nLocation: {location}\r\n"
        f"Content-Length: {length}\r\nContent-Encoding: unneeded-coding\r\n\r\n".encode()
    )


def mixed_http1(connection, destination):
    first, headers = request(connection)
    assert headers[b"authorization"] == b"Bearer secret"
    assert headers[b"cookie"] == b"session=secret"
    if first == b"POST /start HTTP/1.1":
        assert headers[b"expect"] == b"100-continue"
        assert headers[b"content-length"] == b"7"
        redirect(connection, 302, "/next?x=1,2")
    else:
        assert first == b"GET /next?x=1,2 HTTP/1.1"
        assert headers[b"x-keep"] == b"kept"
        for name in (b"content-type", b"content-encoding", b"trailer", b"expect"):
            assert name not in headers
        assert headers.get(b"content-length", b"0") == b"0"
        redirect(connection, 307, destination, 100)
    closed(connection)


def hpack_field(name, value):
    assert len(name) < 127 and len(value) < 127
    return b"\x00" + bytes([len(name)]) + name + bytes([len(value)]) + value


def mixed_http2(connection):
    assert connection.selected_alpn_protocol() == "h2"
    send_frame(connection, 4, 0, 0)
    assert exact(connection, 24) == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    received_headers = False
    while True:
        header = exact(connection, 9)
        count = int.from_bytes(header[:3], "big")
        kind, flags = header[3:5]
        stream = int.from_bytes(header[5:], "big")
        assert count <= 65536
        payload = exact(connection, count)
        if kind == 4:
            assert stream == 0
            if not flags & 1:
                send_frame(connection, 4, 1, 0)
        elif kind == 8:
            assert stream == 0
        elif kind == 1:
            assert stream == 1 and flags & 4
            if received_headers:
                assert not payload
            else:
                assert payload
                received_headers = True
            if flags & 1:
                break
        else:
            assert kind == 0 and stream == 1 and received_headers and not payload
            if flags & 1:
                break
    body = gzip.compress(b"redirect done", mtime=0)
    fields = b"\x88" + hpack_field(b"content-encoding", b"gzip")
    fields += hpack_field(b"content-length", str(len(body)).encode())
    send_frame(connection, 1, 4, 1, fields)
    send_frame(connection, 0, 0, 1, body)
    send_frame(connection, 1, 5, 1, hpack_field(b"x-final", b"h2"))
    closed(connection, h2=True)


def replay_http1(connection):
    for attempt, target in enumerate((b"/first", b"/repeat", b"/final"), 1):
        first, headers = request(connection)
        assert first == b"POST " + target + b" HTTP/1.1"
        assert headers[b"expect"] == b"100-continue"
        assert headers[b"transfer-encoding"] == b"chunked" and b"content-length" not in headers
        assert headers[b"content-encoding"] == b"gzip"
        assert headers[b"content-type"] == b"text/plain"
        assert headers[b"trailer"] == b"x-attempt"
        assert headers[b"authorization"] == b"Bearer secret"
        assert headers[b"cookie"] == b"session=secret"
        connection.sendall(b"HTTP/1.1 100 Continue\r\n\r\n")
        body, trailers = chunked(connection)
        assert gzip.decompress(body) == b"payload"
        assert trailers == [(b"x-attempt", str(attempt).encode())]
        if attempt == 3:
            response(connection, body=b"replayed")
        else:
            redirect(
                connection, 307 if attempt == 1 else 308, "/repeat" if attempt == 1 else "/final"
            )
    closed(connection)


def network_progress_http1(connection, destination):
    first, headers = request(connection)
    assert first == b"GET /warm HTTP/1.1" and headers[b"content-length"] == b"0"
    connection.sendall(b"HTTP/1.1 204 Warm\r\n\r\n")
    first, headers = request(connection)
    assert first == b"POST /warm HTTP/1.1" and headers[b"content-length"] == b"4"
    assert exact(connection, 4) == b"sent"
    redirect(connection, 307, destination)
    closed(connection)


def listener():
    result = socket.socket()
    result.bind(("127.0.0.1", 0))
    result.listen(4)
    result.settimeout(180)
    return result


def exercise(binary, mode, context):
    failures = []
    with listener() as first, listener() as second:
        start = f"http://127.0.0.1:{first.getsockname()[1]}"
        if mode == "mixed":
            destination = f"https://localhost:{second.getsockname()[1]}/final#"
            handlers = [
                (first, lambda connection: mixed_http1(connection, destination), None, 4),
                (second, mixed_http2, context, 2),
            ]
            start += "/start#saved"
        elif mode == "replay":
            destination = start + "/final#keep"
            start += "/first#keep"
            handlers = [(first, replay_http1, None, 2)]
        else:
            assert mode == "progress"
            destination = f"http://127.0.0.1:{second.getsockname()[1]}/next"
            start += "/warm"
            handlers = [
                (first, lambda connection: network_progress_http1(connection, destination), None, 2)
            ]

        def worker(server, handler, tls):
            try:
                with server.accept()[0] as raw:
                    raw.settimeout(180)
                    if tls is None:
                        handler(raw)
                    else:
                        with tls.wrap_socket(raw, server_side=True) as secure:
                            handler(secure)
            except BaseException as error:
                traceback.print_exc()
                failures.append(error)

        workers = [
            threading.Thread(target=worker, args=handler[:3], daemon=True)
            for handler in handlers
            for _ in range(handler[3])
        ]
        for thread in workers:
            thread.start()
        result = subprocess.run(
            [binary, mode, start, destination],
            capture_output=True,
            text=True,
            timeout=180,
            env=os.environ,
        )
        for thread in workers:
            thread.join(180)
            assert not thread.is_alive(), "redirect peer did not finish"
        if failures:
            raise failures[0]
        assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
        assert result.stdout == f"redirects {mode}\n" and not result.stderr
        if mode == "progress":
            assert not select.select([second], [], [], 0)[0]


def pem_function(path, name):
    text = path.read_text().split("public fun " + name + "()", 1)[1].split("public fun ", 1)[0]
    return "".join(ast.literal_eval(value) for value in re.findall(r'"(?:[^"\\]|\\.)*"', text))


def main():
    binary, server_source, intermediate_source, output = sys.argv[1:]
    directory = Path(output)
    directory.mkdir(parents=True, exist_ok=True)
    chain, key = directory / "chain.pem", directory / "key.pem"
    chain.write_text(
        pem_function(Path(server_source), "serverPem")
        + pem_function(Path(intermediate_source), "intermediatePem")
    )
    key.write_text(pem_function(Path(server_source), "serverKeyPem"))
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(chain, key)
    context.set_alpn_protocols(["h2"])
    for mode in ("mixed", "replay", "progress"):
        exercise(binary, mode, context)
    print("independent HTTP redirects ALPN gzip replay and network progress passed")


if __name__ == "__main__":
    main()
