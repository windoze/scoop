import collections
import contextlib
import gzip
import http.server

from peer_digest import authorize, response_info

PAYLOAD = b"0123456789abcdef" * 3
ENCODED = gzip.compress(PAYLOAD, mtime=0)
REPLACEMENT = b"replacement complete entity"
COUNTS = collections.Counter()
FAILURES = []
TRACES = collections.defaultdict(list)


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

    def respond(self):
        mode = self.path.split("/")[1]
        assert self.headers.get("Accept-Encoding") == ("gzip" if mode == "raw" else "identity")
        auth = authorize(self) if mode.startswith("digest-") else None
        if mode.startswith("digest-") and auth is None:
            return
        COUNTS[mode] += 1
        stage = COUNTS[mode]
        requested = self.headers.get("Range")
        TRACES[mode].append(requested)
        body = ENCODED if mode == "raw" else PAYLOAD
        fields = [("ETag", '"v1"')]
        if mode == "raw":
            fields.append(("Content-Encoding", "gzip"))
        if auth is not None:
            assert auth[0]["nonce"] == (
                "initial"
                if stage <= 2 and mode == "digest-incomplete"
                else "initial"
                if stage == 1
                else "next"
            )
        warm = mode == "cache" and stage == 1
        if warm or stage == (2 if mode == "cache" else 1):
            assert requested is None and self.headers.get("If-Range") is None
            fields.append(("Cache-Control", "max-age=3600"))
            if auth is not None:
                fields.append(response_info(auth, self.path, body))
            complete = warm or mode in ("complete-416", "bad-416", "digest-prefix")
            self.reply(
                200,
                body,
                fields,
                sent=body if complete else body[:8],
                chunked=mode == "unknown-size",
                complete=complete,
            )
            return
        if mode == "digest-incomplete" or (mode == "mismatch" and stage == 3):
            assert requested is None and self.headers.get("If-Range") is None
            body = REPLACEMENT if mode == "mismatch" else body
            if auth is not None:
                fields.append(response_info(auth, self.path, body))
            self.reply(200, body, fields)
            return
        offset = (
            48
            if mode in ("complete-416", "bad-416")
            else 24
            if mode == "segments" and stage == 3
            else 8
        )
        assert requested == f"bytes={offset}-" and self.headers.get("If-Range") == '"v1"'
        if mode == "cookie":
            assert self.headers.get("Cookie") == "session=two"
        if mode == "authorization":
            assert self.headers.get("Authorization") == "Bearer two"
        if mode in ("restart", "fail-200"):
            self.reply(200, REPLACEMENT, [("ETag", '"v2"')])
            return
        if mode == "status":
            self.reply(404, b"missing", [])
            return
        if mode in ("complete-416", "bad-416"):
            self.reply(
                416,
                b"already complete",
                fields
                + [
                    ("Content-Range", f"bytes */{48 if mode == 'complete-416' else 49}"),
                    ("Content-Encoding", "gzip"),
                ],
            )
            return
        if mode == "final-url" and stage == 2:
            self.reply(302, b"", [("Location", "/final-url/moved")])
            return
        last = (
            23
            if mode == "segments" and stage == 2
            else 27
            if mode == "digest-prefix" and stage == 2
            else len(body) - 1
        )
        part = body[offset : last + 1]
        if mode in ("fail-tag", "mismatch"):
            fields = [("ETag", '"v2"')]
        if mode == "encoding":
            fields.append(("Content-Encoding", "gzip"))
        first = offset + 1 if mode == "start" else offset
        total = "*" if mode == "unknown-range" else str(len(body) + (1 if mode == "total" else 0))
        fields.append(("Content-Range", f"bytes {first}-{last}/{total}"))
        if auth is not None:
            fields.append(response_info(auth, self.path, part))
        sent = part[:-3] if mode == "short" else part + b"!" if mode == "long" else part
        if mode == "truncated-range" or (mode == "digest-prefix" and stage == 2):
            sent = part[:20] if mode == "truncated-range" else part[:8]
        self.reply(
            206,
            part,
            fields,
            sent=sent,
            chunked=mode in ("short", "long"),
            complete=mode not in ("truncated-range",)
            and not (mode == "digest-prefix" and stage == 2),
            extra_length=1 if mode == "length" else 0,
        )

    def reply(
        self, status, body, fields, *, sent=None, chunked=False, complete=True, extra_length=0
    ):
        self.send_response(status)
        self.send_header(
            "Transfer-Encoding" if chunked else "Content-Length",
            "chunked" if chunked else str(len(body) + extra_length),
        )
        for name, value in fields:
            self.send_header(name, value)
        self.end_headers()
        sent = body if sent is None else sent
        if chunked:
            self.wfile.write(f"{len(sent):x}\r\n".encode() + sent + b"\r\n")
            if complete:
                self.wfile.write(b"0\r\n\r\n")
        else:
            self.wfile.write(sent)
        self.wfile.flush()
        if not complete:
            self.close_connection = True
