"""Exception previews retain real handlers and flow from complete regions."""


def check(peer, root):
    path = root / "src/exceptionRecovery.scoop"
    source = (
        "class LspFailure<T>(val payload: T) : Exception()\n"
        "fun catchChoice(value: String, next: Boolean): Boolean = next\n"
        "fun catchChoice(value: Boolean, next: Boolean): Boolean = next\n"
        "fun catchHover() { try {} catch (error: LspFailure<String>) { error. } }\n"
        "fun catchSignature() { try {} catch (error: LspFailure<String>) { catchChoice(error.payload, } }\n"
        "fun tryMutable(value: Any) { var local: Any = value; if (local is String) { try { local. } catch (error: Throwable) { local. } finally { local. } } }\n"
        "fun tryFinally(value: Any) { var local: Any = value; if (local is String) { try {} finally { local. } } }\n"
        "fun tryRejected() { try {} catch (error: String) { error. } }\n"
    )
    path.write_text(source)
    uri = path.as_uri()
    peer.notify(
        "textDocument/didOpen",
        {"textDocument": {"uri": uri, "languageId": "scoop", "version": 1, "text": source}},
    )

    def query(method, line, marker, after=False):
        offset = source.splitlines()[line].rindex(marker)
        if after:
            offset += len(marker)
        return peer.request(
            method,
            {"textDocument": {"uri": uri}, "position": {"line": line, "character": offset}},
        )

    def check_type(expected):
        hover = query("textDocument/hover", 3, "error.")
        assert hover["contents"]["value"] == f"error: LspFailure<{expected}>", hover
        result = query("textDocument/signatureHelp", 4, ",", after=True)
        assert result and len(result["signatures"]) == 1, result
        assert result["signatures"][0]["parameters"][0]["label"] == f"value: {expected}", result

    check_type("String")
    definition = query("textDocument/definition", 3, "error.")
    assert definition["uri"] == uri, definition
    assert definition["range"]["start"] == {
        "line": 3,
        "character": source.splitlines()[3].index("error:"),
    }, definition
    completion = query("textDocument/completion", 3, "error.", after=True)
    assert any(item["label"] == "payload" for item in completion["items"]), completion
    for line, expected in [(5, "Any"), (6, "String")]:
        hover = query("textDocument/hover", line, "local.")
        assert hover["contents"]["value"] == f"local: {expected}", hover
    assert query("textDocument/hover", 7, "error.") is None
    disk_source = source
    source = source.replace("LspFailure<String>", "LspFailure<Boolean>")
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 3}, "contentChanges": [{"text": source}]},
    )
    check_type("Boolean")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    source = disk_source
    check_type("String")
    assert not (root / "target").exists()
