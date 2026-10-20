import ast
import http.server
import re
import ssl
import subprocess
import sys
import threading
from pathlib import Path


def pem_function(path: Path, name: str) -> str:
    text = path.read_text().split("public fun " + name + "()", 1)[1]
    text = text.split("public fun ", 1)[0]
    return "".join(ast.literal_eval(value) for value in re.findall(r'"(?:[^"\\]|\\.)*"', text))


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, _format, *_args):
        pass

    def do_PATCH(self):
        try:
            assert self.path == "/external?q=one&q=two"
            assert self.headers.get_all("X-Repeat") == ["one", "two"]
            assert self.headers.get_content_type() == "text/plain"
            assert self.headers.get_content_charset() == "utf-8"
            expected = "external body 中文".encode()
            assert int(self.headers["Content-Length"]) == len(expected)
            assert self.rfile.read(len(expected)) == expected
            self.send_response_only(103)
            self.send_header("Link", "</style.css>; rel=preload")
            self.end_headers()
            body = "Python response 中文".encode()
            self.send_response_only(207, "Independent response")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(body[:3])
            self.wfile.flush()
            self.wfile.write(body[3:])
            self.wfile.flush()
            self.server.count += 1
        except BaseException as error:
            self.server.failures.append(repr(error))
        finally:
            self.close_connection = True


def run(executable: str, context: ssl.SSLContext | None, label: str):
    server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
    server.count = 0
    server.failures = []
    if context is not None:
        server.socket = context.wrap_socket(server.socket, server_side=True)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    scheme = "http" if context is None else "https"
    host = "127.0.0.1" if context is None else "localhost"
    url = f"{scheme}://{host}:{server.server_port}/external?q=one&q=two#local"
    try:
        result = subprocess.run(
            [executable, "external", url],
            capture_output=True,
            text=True,
            timeout=90,
            check=False,
        )
        assert result.returncode == 0, (label, result.returncode, result.stdout, result.stderr)
        assert result.stdout == "external HTTP1 client exchange passed\n", result.stdout
        assert result.stderr == "", result.stderr
        assert server.count == 1 and not server.failures, (server.count, server.failures)
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


def main():
    executable, server_source, intermediate_source, output = sys.argv[1:]
    directory = Path(output)
    directory.mkdir(parents=True, exist_ok=True)
    chain = directory / "chain.pem"
    key = directory / "key.pem"
    chain.write_text(
        pem_function(Path(server_source), "serverPem")
        + pem_function(Path(intermediate_source), "intermediatePem")
    )
    key.write_text(pem_function(Path(server_source), "serverKeyPem"))
    run(executable, None, "plain")
    for alpn in (True, False):
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(chain, key)
        if alpn:
            context.set_alpn_protocols(["http/1.1"])
        run(executable, context, "ALPN" if alpn else "no ALPN")
    print("Python HTTP1 verifies plain TLS ALPN and permitted fallback clients")


if __name__ == "__main__":
    main()
