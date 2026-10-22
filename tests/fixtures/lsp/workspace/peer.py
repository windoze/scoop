"""Real LSP peer for project configuration and precompiled dependency inputs."""

import json
import queue
import subprocess
import sys
import threading
from pathlib import Path


class Peer:
    def __init__(self, server):
        self.process = subprocess.Popen(
            [server, "--stdio"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.messages = queue.Queue()
        self.diagnostics = {}
        self.next_id = 1
        self.registered = False
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        try:
            stream = self.process.stdout
            while line := stream.readline():
                headers = {}
                while line not in (b"\r\n", b"\n"):
                    name, value = line.decode("ascii").split(":", 1)
                    headers[name.lower()] = value.strip()
                    line = stream.readline()
                    if not line:
                        raise EOFError("incomplete LSP headers")
                length = int(headers["content-length"])
                data = bytearray()
                while len(data) < length:
                    part = stream.read(length - len(data))
                    if not part:
                        raise EOFError("incomplete LSP message")
                    data.extend(part)
                self.messages.put(json.loads(data))
            self.messages.put(EOFError("LSP stream closed"))
        except BaseException as error:
            self.messages.put(error)

    def send(self, message):
        payload = json.dumps({"jsonrpc": "2.0", **message}, ensure_ascii=False).encode()
        frame = f"Content-Length: {len(payload)}\r\n\r\n".encode() + payload
        for offset in range(0, len(frame), 11):
            self.process.stdin.write(frame[offset : offset + 11])
            self.process.stdin.flush()

    def notify(self, method, params):
        self.send({"method": method, "params": params})

    def request(self, method, params):
        request_id = self.next_id
        self.next_id += 1
        self.send({"id": request_id, "method": method, "params": params})
        while True:
            message = self.messages.get(timeout=20)
            if isinstance(message, BaseException):
                raise message
            if message.get("method") == "client/registerCapability":
                registrations = message["params"]["registrations"]
                assert registrations[0]["method"] == "workspace/didChangeWatchedFiles"
                self.registered = True
                self.send({"id": message["id"], "result": None})
            elif message.get("method") == "textDocument/publishDiagnostics":
                self.diagnostics[message["params"]["uri"]] = message["params"]
            elif message.get("id") == request_id:
                assert "error" not in message, message
                return message["result"]
            else:
                raise AssertionError(message)

    def finish(self):
        assert self.request("shutdown", None) is None
        self.notify("exit", None)
        assert self.process.wait(timeout=10) == 0
        assert self.process.stderr.read() == b""


def main():
    server, root, sysroot, target = sys.argv[1:]
    root = Path(root)
    manifest = (root / "Cone.toml").as_uri()
    source = (root / "src/value.scoop").as_uri()
    peer = Peer(server)
    try:
        options = {"target": target, "sysroot": sysroot}
        peer.request(
            "initialize",
            {
                "processId": None,
                "workspaceFolders": [{"name": "project", "uri": root.as_uri()}],
                "initializationOptions": options,
                "capabilities": {
                    "workspace": {"didChangeWatchedFiles": {"dynamicRegistration": True}},
                    "textDocument": {"documentSymbol": {"hierarchicalDocumentSymbolSupport": True}},
                },
            },
        )
        peer.notify("initialized", {})
        peer.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": source,
                    "version": 1,
                    "languageId": "scoop",
                    "text": 'fun greeting(): String = "中文😀"\n',
                }
            },
        )

        def symbols():
            result = peer.request("textDocument/documentSymbol", {"textDocument": {"uri": source}})
            assert result[0]["name"] == "greeting", result

        symbols()
        assert peer.registered
        assert not peer.diagnostics.get(manifest, {}).get("diagnostics"), peer.diagnostics
        other = (
            "x86_64-unknown-linux-gnu"
            if target == "aarch64-apple-darwin"
            else "aarch64-apple-darwin"
        )
        peer.notify(
            "workspace/didChangeConfiguration",
            {"settings": {"scoop": {**options, "target": other}}},
        )
        symbols()
        assert peer.diagnostics[manifest]["diagnostics"][0]["code"] == "SCOOP_PROJECT"
        peer.notify(
            "workspace/didChangeConfiguration",
            {"settings": {"scoop": {**options, "purpose": "test"}}},
        )
        symbols()
        assert peer.diagnostics[manifest]["diagnostics"] == []
        peer.notify(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": manifest,
                    "version": 1,
                    "languageId": "toml",
                    "text": "schema = broken\n",
                }
            },
        )
        symbols()
        assert peer.diagnostics[manifest]["version"] == 1
        assert peer.diagnostics[manifest]["diagnostics"][0]["code"] == "SCOOP_PROJECT"
        peer.notify(
            "textDocument/didChange",
            {
                "textDocument": {"uri": manifest, "version": 2},
                "contentChanges": [{"text": (root / "Cone.toml").read_text()}],
            },
        )
        symbols()
        assert peer.diagnostics[manifest]["version"] == 2
        assert peer.diagnostics[manifest]["diagnostics"] == []
        peer.finish()
    finally:
        if peer.process.poll() is None:
            peer.process.kill()
            peer.process.wait(timeout=10)
    print("workspace inputs passed")


if __name__ == "__main__":
    main()
