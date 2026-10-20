import contextlib
import gzip
import os
import socket
import subprocess
import sys
import threading

import wire

STRESS = any(
    os.environ.get(name) == "1" for name in ("SCOOP_GC_STRESS_MOVE", "SCOOP_GC_STRESS_MINOR")
)
TIMEOUT = 1800 if STRESS else 180


def reply(connection, status=200, body=b"done", fields=()):
    header = (
        f"HTTP/1.1 {status} Cookie\r\nContent-Length: {len(body)}\r\nConnection: close\r\n".encode()
    )
    for name, value in fields:
        header += name + b": " + value + b"\r\n"
    connection.sendall(header + b"\r\n" + body)


def expect(connection, target, cookie=None):
    first, fields = wire.request(connection)
    assert first == target, first
    assert fields.get(b"cookie") == cookie, fields
    assert b"proxy-authorization" not in fields
    return fields


def sequence(listener):
    specs = (
        (b"POST /start HTTP/1.1", None, 302, b"sid=redirect; Path=/"),
        (b"GET /auth HTTP/1.1", b"sid=redirect", 401, b"auth=A; Path=/"),
        (b"GET /auth HTTP/1.1", b"sid=redirect; auth=A", 503, b"retry=R; Path=/"),
        (b"GET /auth HTTP/1.1", b"sid=redirect; auth=A; retry=R", 200, b"final=F; Path=/"),
        (b"GET /next HTTP/1.1", b"sid=redirect; auth=A; retry=R; final=F", 200, None),
    )
    for index, (target, cookie, status, setting) in enumerate(specs):
        with listener.accept()[0] as connection:
            connection.settimeout(TIMEOUT)
            fields = expect(connection, target, cookie)
            if index in (2, 3):
                scheme, token = fields[b"authorization"].split(b" ", 1)
                assert scheme.lower() == b"basic" and token == b"dXNlcjpwYXNz"
            else:
                assert b"authorization" not in fields
            extra = [] if setting is None else [(b"Set-Cookie", setting)]
            if index == 0:
                connection.sendall(
                    b"HTTP/1.1 103 Early Hints\r\nSet-Cookie: early=ignored; Path=/\r\n\r\n"
                )
                extra.append((b"Location", b"/auth"))
            if index == 1:
                extra.append((b"WWW-Authenticate", b'Basic realm="cookies"'))
            body = b"done" if status == 200 else b""
            if index == 3:
                body = gzip.compress(body, mtime=0)
                extra.append((b"Content-Encoding", b"gzip"))
            reply(connection, status, body, extra)


def manual(first, second, other):
    for target, location in (
        (b"GET /start HTTP/1.1", b"/same"),
        (b"GET /same HTTP/1.1", other.encode()),
    ):
        with first.accept()[0] as connection:
            expect(connection, target, b"manual=M")
            reply(
                connection,
                302,
                b"",
                ((b"Location", location), (b"Set-Cookie", b"origin=O; Path=/")),
            )
    with second.accept()[0] as connection:
        expect(connection, b"GET /next HTTP/1.1", b"other=2")
        reply(connection)


def disabled(listener):
    with listener.accept()[0] as connection:
        expect(connection, b"POST /start HTTP/1.1")
        reply(
            connection, 303, b"", ((b"Location", b"/next"), (b"Set-Cookie", b"blocked=B; Path=/"))
        )
    with listener.accept()[0] as connection:
        expect(connection, b"GET /next HTTP/1.1")
        reply(connection, fields=((b"Set-Cookie", b"ignored=I; Path=/"),))


def proxy(listener):
    for index in range(3):
        with listener.accept()[0] as connection:
            expect(
                connection,
                b"GET http://origin.invalid/resource HTTP/1.1",
                b"seed=1" if index < 2 else b"seed=1; origin=O",
            )
            if index == 0:
                reply(
                    connection,
                    407,
                    b"",
                    (
                        (b"Proxy-Authenticate", b'Basic realm="proxy"'),
                        (b"Set-Cookie", b"attack=A; Path=/"),
                    ),
                )
            elif index == 1:
                reply(connection, fields=((b"Set-Cookie", b"origin=O; Path=/"),))
            else:
                reply(connection)


def run(binary, args):
    result = subprocess.run([binary, *args], capture_output=True, text=True, timeout=TIMEOUT + 30)
    assert (
        result.returncode == 0
        and result.stdout == f"client cookie {args[0]}\n"
        and not result.stderr
    ), (args, result)


def external(binary, mode):
    failures = []
    with contextlib.ExitStack() as stack:
        first, second = (stack.enter_context(socket.socket()) for _ in range(2))
        for listener in (first, second):
            listener.bind(("127.0.0.1", 0))
            listener.listen(8)
            listener.settimeout(TIMEOUT)
        target = f"http://127.0.0.1:{first.getsockname()[1]}/" + (
            "" if mode == "proxy" else "start"
        )
        other = f"http://localhost:{second.getsockname()[1]}/next"

        def serve():
            try:
                if mode == "sequence":
                    sequence(first)
                elif mode == "manual":
                    manual(first, second, other)
                elif mode == "proxy":
                    proxy(first)
                else:
                    disabled(first)
            except BaseException as error:
                failures.append(error)

        worker = threading.Thread(target=serve, daemon=True)
        worker.start()
        try:
            run(binary, ["external", mode, target, other])
            worker.join(TIMEOUT)
            assert not worker.is_alive() and not failures, (mode, failures)
        except BaseException as error:
            raise AssertionError((mode, failures, error)) from error
        for listener in (first, second):
            listener.settimeout(0.01)
            try:
                extra, _ = listener.accept()
            except TimeoutError:
                continue
            extra.close()
            raise AssertionError((mode, "unexpected extra request"))


def main():
    binary = sys.argv[1]
    for mode in ("sequence", "manual", "disabled", "default", "proxy"):
        external(binary, mode)
    run(binary, ["http2"])
    for stage in ("load", "save"):
        run(binary, ["failure", stage])
        for mode in ("caller", "client", "total"):
            run(binary, ["cancel", stage, mode, "normal"])
        run(binary, ["cancel", stage, "client", "cleanup"])
    print("client cookies network authentication redirects retries H2 and callback lifetime passed")


if __name__ == "__main__":
    main()
