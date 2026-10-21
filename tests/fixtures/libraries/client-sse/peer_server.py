import collections
import contextlib
import gzip
import http.server

COUNTS = collections.Counter()
FAILURES = []


class Peer(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def handle(self):
        with contextlib.suppress(ConnectionResetError):
            super().handle()

    def do_GET(self):
        try:
            self.respond()
        except (BrokenPipeError, ConnectionResetError):
            pass
        except BaseException as error:
            FAILURES.append((self.path, error))
            self.close_connection = True

    do_POST = do_GET

    def respond(self):
        mode = self.path.split("/")[1]
        if self.path.endswith("/probe"):
            COUNTS[mode + "/probe"] += 1
            self.reply(200, b"ok", [])
            return
        if mode == "redirect" and not self.path.endswith("/events"):
            self.reply(302, b"", [("Location", "/redirect/events")])
            return
        COUNTS[mode] += 1
        stage = COUNTS[mode]
        if mode == "cache" and stage == 1:
            self.reply(200, b"cached", [("Cache-Control", "max-age=3600")])
            return
        assert self.headers.get("Accept") == (
            "custom/type" if mode == "header" else "text/event-stream"
        )
        assert self.headers.get("Cache-Control") == (
            "max-age=1" if mode == "header" else "no-cache"
        )
        assert "gzip" in self.headers.get("Accept-Encoding", "")
        expected_id = (
            "header" if mode == "header" else "seed" if stage == 1 or mode == "cache" else "first"
        )
        if stage == 3 and mode in ("reconnect", "post-reconnect", "cookie"):
            expected_id = None
        assert self.headers.get("Last-Event-ID") == expected_id, (mode, stage, self.headers)
        if mode in ("post", "post-reconnect", "post-once"):
            assert self.command == "POST" and self.headers.get("Expect") == "100-continue"
            assert self.rfile.read(int(self.headers["Content-Length"])) == b"posted"
        else:
            assert self.command == "GET"
        if mode == "cookie":
            assert self.headers.get("Cookie") == (
                None if stage == 1 else "session=one" if stage == 2 else "session=two"
            )
        if (
            mode == "stop"
            or stage == 3
            or (stage == 2 and mode in ("truncated", "callback-close", "redirect"))
        ):
            self.reply(204, b"", [])
            return
        if mode == "informational-error" and stage == 2:
            self.wfile.write(b"HTTP/1.1 103 Early Hints\r\nLink: </x>\r\n\r\n")
            self.wfile.flush()
            self.close_connection = True
            return
        fields = [("Content-Type", "text/event-stream; charset=us-ascii")]
        if mode == "missing":
            fields = []
        elif mode == "media":
            fields = [("Content-Type", "text/plain")]
        elif mode == "duplicate":
            fields += fields
        elif mode == "bad-media":
            fields = [("Content-Type", 'text/event-stream; charset="unfinished')]
        if mode in ("plain", "gzip"):
            body = (
                b"\xef\xbb\xbf: comment\r\nid: first\revent: price\ndata: hello\r\ndata: "
                + "雪".encode()
                + b"\r\n\r\ndata: bad:\xff\n\nid: tail\n\ndata: incomplete"
            )
        elif mode == "line-limit":
            body = b"data: " + b"x" * 32 + b"\n\n"
        elif stage == 2 and mode in ("reconnect", "post-reconnect", "cookie"):
            body = b"id:\nretry: 0\ndata: two\n\n"
        elif stage == 2 and mode == "limit":
            body = b"data: two\n\n"
        else:
            body = b"id: first\nretry: 999999999999\ndata: one\n\n"
        if mode == "gzip":
            body = gzip.compress(body, mtime=0)
            fields.append(("Content-Encoding", "gzip"))
        elif mode == "bad-gzip":
            body = b"broken"
            fields.append(("Content-Encoding", "gzip"))
        if mode == "cookie":
            fields.append(
                ("Set-Cookie", "session=" + ("one" if stage == 1 else "two") + "; Path=/")
            )
        status = 404 if mode == "status" else 201 if mode == "other-success" else 200
        self.reply(
            status, body, fields, truncated=mode == "truncated", bad_chunk=mode == "bad-chunk"
        )

    def reply(self, status, body, fields, *, truncated=False, bad_chunk=False):
        self.send_response(status)
        self.send_header(
            "Transfer-Encoding" if bad_chunk else "Content-Length",
            "chunked" if bad_chunk else str(len(body) + (10 if truncated else 0)),
        )
        for name, value in fields:
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(b"ZZ\r\n" if bad_chunk else body)
        self.wfile.flush()
        if truncated or bad_chunk:
            self.close_connection = True
