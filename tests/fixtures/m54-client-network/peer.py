import ast
import os
import re
import socket
import ssl
import subprocess
import sys
import threading
from pathlib import Path


def pem_function(path, name):
    text = path.read_text().split("public fun " + name + "()", 1)[1]
    text = text.split("public fun ", 1)[0]
    return "".join(ast.literal_eval(value) for value in re.findall(r'"(?:[^"\\]|\\.)*"', text))


def exact(connection, size):
    result = bytearray()
    while len(result) < size:
        part = connection.recv(size - len(result))
        assert part, "truncated ClientHello"
        result += part
    return bytes(result)


def serve_http(connection, mode):
    with connection.makefile("rb") as source:
        first = None
        for _ in range(2):
            line = source.readline(8192)
            allowed = (
                (b"GET /reuse HTTP/1.1\r\n",)
                if mode == "reuse"
                else (b"GET /dns HTTP/1.1\r\n", b"GET /ip HTTP/1.1\r\n")
            )
            assert line in allowed, line
            if first is None:
                first = line
            else:
                assert line == first, "the second exchange did not reuse the same origin"
            while True:
                field = source.readline(8192)
                assert field.endswith(b"\r\n"), field
                if field == b"\r\n":
                    break
            connection.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
        assert source.read(1) == b""


def exercise(binary, mode, context):
    failures = []
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(8)
        listener.settimeout(180)

        def worker():
            try:
                with listener.accept()[0] as raw:
                    raw.settimeout(180)
                    if mode == "reuse":
                        with context.wrap_socket(raw, server_side=True) as secure:
                            assert secure.selected_alpn_protocol() == "http/1.1"
                            serve_http(secure, mode)
                    elif mode == "clear":
                        serve_http(raw, mode)
                    elif mode == "zero-tls":
                        assert raw.recv(1) == b"", "zero TLS deadline sent handshake bytes"
                    else:
                        header = exact(raw, 5)
                        assert header[0] == 22
                        payload = exact(raw, int.from_bytes(header[3:5], "big"))
                        assert payload[0] == 1
                        while raw.recv(8192):
                            pass
            except BaseException as error:
                failures.append(error)

        count = 4 if mode == "clear" else 2
        workers = [threading.Thread(target=worker, daemon=True) for _ in range(count)]
        for thread in workers:
            thread.start()
        result = subprocess.run(
            [binary, mode, str(listener.getsockname()[1])],
            capture_output=True,
            text=True,
            timeout=180,
            env=dict(os.environ, SCOOP_GC_STRESS_MOVE="1"),
        )
        for thread in workers:
            thread.join(180)
            assert not thread.is_alive(), "network deadline peer did not finish"
        if failures:
            raise failures[0]
        assert result.returncode == 0, (mode, result.returncode, result.stdout, result.stderr)
        assert result.stdout == f"network {mode}\n" and not result.stderr


def main():
    binary, server_source, intermediate_source, output = sys.argv[1:]
    directory = Path(output)
    directory.mkdir(parents=True, exist_ok=True)
    chain = directory / "chain.pem"
    key = directory / "key.pem"
    chain.write_text(
        pem_function(Path(server_source), "serverPem")
        + pem_function(Path(intermediate_source), "intermediatePem")
    )
    key.write_text(pem_function(Path(server_source), "serverKeyPem"))
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(chain, key)
    context.set_alpn_protocols(["http/1.1"])
    for mode in ("clear", "zero-tls", "tls", "total", "reuse"):
        exercise(binary, mode, context)
    print("independent DNS TCP and TLS deadlines passed")


if __name__ == "__main__":
    main()
