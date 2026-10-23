"""Real LSP peer for project configuration and precompiled dependency inputs."""

import json
import queue
import subprocess
import sys
import threading
from pathlib import Path

from accessor_bodies import check as check_accessor_bodies
from callable_arguments import check as check_callable_arguments
from callable_literals import check as check_callable_literals
from callable_values import check as check_callable_values
from cancellation import check as check_cancellation
from constructor_references import check as check_constructor_references
from diagnostics import check as check_diagnostics
from do_while_recovery import check as check_do_while_recovery
from documentation import check as check_documentation
from exception_recovery import check as check_exception_recovery
from file_headers import check as check_file_headers
from for_recovery import check as check_for_recovery
from import_references import check as check_import_references
from incomplete_signatures import check as check_incomplete_signatures
from local_function_recovery import check as check_local_function_recovery
from member_implementations import check as check_member_implementations
from property_references import check as check_property_references
from qualified_signatures import check as check_qualified_signatures
from recovered_classes import check as check_recovered_classes
from recovered_enums import check as check_recovered_enums
from recovered_interfaces import check as check_recovered_interfaces
from recovered_objects import check as check_recovered_objects
from recovered_structs import check as check_recovered_structs
from references import check as check_references
from signatures import check as check_signatures
from source_dependencies import check as check_source_dependencies
from type_definitions import check as check_type_definitions
from type_hovers import check as check_type_hovers
from type_implementations import check as check_type_implementations
from type_references import check as check_type_references
from when_recovery import check as check_when_recovery
from while_recovery import check as check_while_recovery


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
        self.diagnostic_versions = {}
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
        return self.response(self.send_request(method, params))

    def send_request(self, method, params):
        request_id = self.next_id
        self.next_id += 1
        self.send({"id": request_id, "method": method, "params": params})
        return request_id

    def response(self, request_id, error_code=None):
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
                params = message["params"]
                self.diagnostics[params["uri"]] = params
                self.diagnostic_versions.setdefault(params["uri"], []).append(params.get("version"))
            elif message.get("id") == request_id:
                if error_code is not None:
                    assert message["error"]["code"] == error_code, message
                    return message["error"]
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
        initialized = peer.request(
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
        assert initialized["capabilities"]["hoverProvider"]
        assert initialized["capabilities"]["definitionProvider"]
        assert initialized["capabilities"]["referencesProvider"]
        assert initialized["capabilities"]["completionProvider"]["triggerCharacters"] == ["."]
        assert initialized["capabilities"]["signatureHelpProvider"]["triggerCharacters"] == [
            "(",
            ",",
        ]
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

        def hover(line, character):
            return peer.request(
                "textDocument/hover",
                {
                    "textDocument": {"uri": source},
                    "position": {"line": line, "character": character},
                },
            )

        def definition(line, character):
            return peer.request(
                "textDocument/definition",
                {
                    "textDocument": {"uri": source},
                    "position": {"line": line, "character": character},
                },
            )

        def completion(line, character):
            return peer.request(
                "textDocument/completion",
                {
                    "textDocument": {"uri": source},
                    "position": {"line": line, "character": character},
                },
            )

        assert hover(0, 5)["contents"]["value"] == "fun greeting(): String"
        edited = (
            'fun greeting(): String = "中文😀"\n'
            "fun damaged(value: Missing): Boolean = true\n"
            "fun <T> editing(value: T): T { value. }\n"
            'fun prefix(value: Boolean) { val value = "local"\nvalue. }\n'
            "fun inferred(value: Boolean) ="
        )
        peer.notify(
            "textDocument/didChange",
            {
                "textDocument": {"uri": source, "version": 2},
                "contentChanges": [{"text": edited}],
            },
        )
        assert hover(0, 5)["contents"]["value"] == "fun greeting(): String"
        assert hover(1, 5) is None
        assert hover(2, 10)["contents"]["value"] == "fun <T> editing(value: T): T"
        parameter = hover(2, 18)
        assert parameter["contents"]["value"] == "value: T", parameter
        assert parameter["range"]["start"] == {"line": 2, "character": 16}
        receiver = hover(2, 32)
        assert receiver["contents"]["value"] == "value: T", receiver
        assert receiver["range"]["start"] == {"line": 2, "character": 31}
        assert hover(4, 2)["contents"]["value"] == "value: String"
        candidates = completion(4, len("value."))
        names = {item["label"] for item in candidates["items"]}
        assert "toString" in names, candidates
        selected = next(item for item in candidates["items"] if item["label"] == "toString")
        assert selected["textEdit"] == {
            "range": {"start": {"line": 4, "character": 6}, "end": {"line": 4, "character": 6}},
            "newText": "toString",
        }
        destination = definition(2, 32)
        assert destination["uri"] == source
        assert destination["range"]["start"] == {"line": 2, "character": 16}
        destination = definition(4, 2)
        assert destination["range"]["start"] == {
            "line": 3,
            "character": edited.splitlines()[3].index("value ="),
        }
        assert (
            hover(5, 5)["contents"]["value"] == "fun inferred(value: Boolean): <pending inference>"
        )
        peer.notify(
            "textDocument/didChange",
            {
                "textDocument": {"uri": source, "version": 3},
                "contentChanges": [
                    {
                        "text": "fun greeting(value: Boolean): Boolean = value\nfun called(): Boolean = helper()\nfun builtin(value: Boolean): String = value.toString()\nfun typed(value: String) { value.toS }\n"
                    }
                ],
            },
        )
        assert hover(0, 5)["contents"]["value"] == "fun greeting(value: Boolean): Boolean"
        body_value = "fun greeting(value: Boolean): Boolean = value".rindex("value")
        assert hover(0, body_value)["contents"]["value"] == "value: Boolean"
        destination = definition(0, body_value)
        assert destination["uri"] == source
        assert destination["range"]["start"] == {"line": 0, "character": 13}
        helper_call = "fun called(): Boolean = helper()".index("helper")
        assert (
            hover(1, helper_call)["contents"]["value"]
            == "fun helper(value: Boolean = true): Boolean"
        )
        destination = definition(1, helper_call)
        assert destination["uri"] == (root / "src/helper.scoop").as_uri()
        assert destination["range"]["start"] == {"line": 0, "character": 4}
        builtin = "fun builtin(value: Boolean): String = value.toString()"
        assert hover(2, builtin.rindex("value"))["contents"]["value"] == "value: Boolean"
        assert definition(2, builtin.index("toString")) is None
        imported_hover = hover(2, builtin.index("toString"))["contents"]["value"]
        assert "toString" in imported_hover and imported_hover.endswith(": String"), imported_hover
        typed = "fun typed(value: String) { value.toS }"
        selector_start = typed.index("toS")
        candidates = completion(3, selector_start + 3)
        selected = next(item for item in candidates["items"] if item["label"] == "toString")
        assert selected["textEdit"]["range"] == {
            "start": {"line": 3, "character": selector_start},
            "end": {"line": 3, "character": selector_start + 3},
        }
        assert hover(3, typed.rindex("value"))["contents"]["value"] == "value: String"
        destination = definition(3, typed.rindex("value"))
        assert destination["range"]["start"] == {"line": 3, "character": typed.index("value")}
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
        assert hover(0, 5) is None
        peer.notify(
            "workspace/didChangeConfiguration",
            {"settings": {"scoop": {**options, "purpose": "test"}}},
        )
        symbols()
        assert peer.diagnostics[manifest]["diagnostics"] == []
        assert hover(0, 5)["contents"]["value"] == "fun greeting(value: Boolean): Boolean"
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
        check_diagnostics(peer, root)
        check_cancellation(peer, root)
        check_signatures(peer, root)
        check_incomplete_signatures(peer, root)
        check_callable_values(peer, root)
        check_property_references(peer, root)
        check_references(peer, root)
        check_import_references(peer, root)
        check_source_dependencies(peer, root)
        check_documentation(peer, root)
        check_constructor_references(peer, root)
        check_accessor_bodies(peer, root)
        check_type_references(peer, root)
        check_type_definitions(peer, root)
        check_type_hovers(peer, root)
        check_type_implementations(peer, root)
        check_member_implementations(peer, root)
        check_recovered_classes(peer, root)
        check_recovered_enums(peer, root)
        check_recovered_objects(peer, root)
        check_qualified_signatures(peer, root)
        check_while_recovery(peer, root)
        check_for_recovery(peer, root)
        check_do_while_recovery(peer, root)
        check_when_recovery(peer, root)
        check_exception_recovery(peer, root)
        check_callable_literals(peer, root)
        check_callable_arguments(peer, root)
        check_local_function_recovery(peer, root)
        check_file_headers(peer, root)
        check_recovered_interfaces(peer, root)
        check_recovered_structs(peer, root)
        peer.finish()
    finally:
        if peer.process.poll() is None:
            peer.process.kill()
            peer.process.wait(timeout=10)
    print("workspace inputs passed")


if __name__ == "__main__":
    main()
