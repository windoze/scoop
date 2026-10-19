import contextlib
import gzip
import os
import socket
import subprocess
import sys
import threading
from pathlib import Path

import oracle
import wire

STRESS = any(
    os.environ.get(name) == "1" for name in ("SCOOP_GC_STRESS_MOVE", "SCOOP_GC_STRESS_MINOR")
)
TIMEOUT = 1800 if STRESS else 180
DATE = b"Tue, 20 Oct 2026 00:00:00 GMT"
LATER = b"Tue, 20 Oct 2026 00:00:10 GMT"


def reply(connection, body=b"", status=200, fields=(), date=DATE):
    head = f"HTTP/1.1 {status} Cache\r\nConnection: close\r\n".encode()
    if status != 304:
        head += f"Content-Length: {len(body)}\r\n".encode()
    if date:
        head += b"Date: " + date + b"\r\n"
    for name, value in fields:
        head += name + b": " + value + b"\r\n"
    connection.sendall(head + b"\r\n" + body)


def serve(connection, mode, index):
    first, fields = wire.request(connection)
    method, target, version = first.split(b" ")
    assert version == b"HTTP/1.1"
    if mode == "digest":
        if index == 0:
            assert b"authorization" not in fields
            reply(
                connection,
                status=401,
                fields=[(b"WWW-Authenticate", oracle.challenge("SHA-256", "auth-int"))],
            )
        else:
            info = oracle.verify(
                fields[b"authorization"], method, target, b"", "SHA-256", "auth-int"
            )
            body = b"authenticated"
            reply(
                connection,
                body,
                fields=[
                    (b"Cache-Control", b"private, max-age=60"),
                    (b"Authentication-Info", oracle.parameters(info(body))),
                ],
            )
        return
    body = b""
    extra = [(b"Cache-Control", b"max-age=60")]
    date = DATE
    status = 200
    if mode == "fresh":
        assert fields[b"accept-encoding"] == b"gzip, deflate, br, zstd"
        body = gzip.compress(b"encoded-cache", mtime=0)
        extra += [(b"Content-Encoding", b"gzip"), (b"Age", b"5"), (b"Vary", b"Accept-Encoding")]
    elif mode in ("etag", "modified", "cookies", "mismatch"):
        if index == 0:
            body = b"original"
            extra = [(b"Cache-Control", b"max-age=1"), (b"X-Meta", b"original")]
            extra += (
                [(b"Last-Modified", b"Mon, 19 Oct 2026 00:00:00 GMT")]
                if mode == "modified"
                else [(b"ETag", b'"v1"')]
            )
        elif index == 1:
            if mode == "modified":
                assert fields[b"if-modified-since"] == b"Mon, 19 Oct 2026 00:00:00 GMT"
                date = None
            else:
                assert fields[b"if-none-match"] == b'"v1"'
                date = LATER
            status = 304
            extra += [(b"X-Meta", b"updated")]
            if mode == "mismatch":
                extra += [(b"ETag", b'"v2"')]
        else:
            assert mode == "mismatch" and b"if-none-match" not in fields
            body = b"changed"
            date = LATER
        if mode == "cookies":
            extra += [(b"Set-Cookie", b"observed=1; Path=/")]
    elif mode == "controls":
        body = (b"first", b"bypass", b"reload", b"private", b"conditional")[index]
        if index == 3:
            assert fields[b"cache-control"] == b"no-store"
        if index == 4:
            assert fields[b"if-none-match"] == b'"manual"'
    elif mode == "vary":
        body = (b"red", b"blue", b"missing", b"empty", b"query-one", b"query-two")[index]
        extra += [(b"Vary", b"X-Mode")]
        if index < 2:
            assert fields[b"x-mode"] == body
        elif index == 2:
            assert b"x-mode" not in fields
        elif index == 3:
            assert fields[b"x-mode"] == b""
        else:
            assert target == (b"/start?a=1&b=2" if index == 4 else b"/start?b=2&a=1")
    elif mode == "nostore":
        body = target.removeprefix(b"/")
        if body == b"no-store":
            extra = [(b"Cache-Control", b"no-store")]
        elif body == b"wildcard":
            extra += [(b"Vary", b"*")]
        elif body == b"invalid":
            extra = [(b"Cache-Control", b"max-age=60, max-age=120")]
        else:
            assert body == b"partial"
            extra += [(b"Content-Range", b"bytes 0-6/10")]
    elif mode == "identity":
        body = (b"A-A", b"B-A", b"A-B", b"isolated", b"isolated-again")[index]
        assert fields[b"authorization"] == (b"Bearer B" if index == 1 else b"Bearer A")
        assert fields[b"cookie"] == (b"user=B" if index == 2 else b"user=A")
    elif mode == "invalidate":
        body = (b"first", b"linked", b"changed", b"updated", b"updated-linked")[index]
        if index == 2:
            assert method == b"POST"
            extra += [(b"Content-Location", b"/linked")]
    elif mode == "abort":
        body = b"complete"
        if index == 2:
            assert target == b"/bad-gzip"
            body = b"invalid-gzip"
            extra += [(b"Content-Encoding", b"gzip")]
    elif mode == "disk":
        body = b"on-disk"
    else:
        raise AssertionError(mode)
    reply(connection, body, status, extra, date)


def run(binary, args):
    result = subprocess.run([binary, *args], capture_output=True, text=True, timeout=TIMEOUT + 30)
    assert (
        result.returncode == 0
        and result.stdout == f"client cache {args[0]}\n"
        and not result.stderr
    ), (
        args,
        result,
    )


def external(binary, mode, count, directory):
    failures = []
    with contextlib.closing(socket.socket()) as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(8)
        listener.settimeout(TIMEOUT)

        def worker():
            try:
                for index in range(count):
                    with listener.accept()[0] as connection:
                        connection.settimeout(TIMEOUT)
                        serve(connection, mode, index)
            except BaseException as error:
                failures.append(error)

        thread = threading.Thread(target=worker, daemon=True)
        thread.start()
        url = f"http://127.0.0.1:{listener.getsockname()[1]}/start"
        try:
            run(binary, [mode, url, str(directory / mode)])
            thread.join(TIMEOUT)
            assert not thread.is_alive() and not failures, failures
        except BaseException as error:
            raise AssertionError((mode, failures, error)) from error
        listener.settimeout(0.01)
        try:
            extra, _ = listener.accept()
        except TimeoutError:
            return
        extra.close()
        raise AssertionError((mode, "unexpected request after expected sequence"))


def main():
    binary, directory = sys.argv[1], Path(sys.argv[2])
    for mode, count in (
        ("fresh", 1),
        ("etag", 2),
        ("modified", 2),
        ("mismatch", 3),
        ("controls", 5),
        ("vary", 6),
        ("nostore", 8),
        ("identity", 5),
        ("invalidate", 5),
        ("abort", 3),
        ("disk", 1),
        ("cookies", 2),
        ("digest", 2),
    ):
        external(binary, mode, count, directory)
    run(binary, ["policies"])
    run(binary, ["http2"])
    print("client cache freshness validation identities streams H2 and lifetime passed")


if __name__ == "__main__":
    main()
