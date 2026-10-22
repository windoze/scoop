"""While recovery keeps real scope and per-iteration condition facts."""


def check(peer, root):
    path = root / "src/whileRecovery.scoop"
    source = (
        "fun whileChoice(value: String, next: Boolean): String = value\n"
        "fun whileChoice(value: Boolean, next: Boolean): Boolean = value\n"
        "fun whileValue(value: Any) { cycle@ while (value is String) { "
        "if (false) continue@cycle; value. } }\n"
        "fun whileSignature(value: Any) { while (value is String) { whileChoice(value, } }\n"
        "fun whileMutable(value: Any) { var local: Any = value; if (local is String) { "
        "while (true) { local.; local = true } } }\n"
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
        hover = query("textDocument/hover", 2, "value.")
        assert hover["contents"]["value"] == f"value: {expected}", hover
        result = query("textDocument/signatureHelp", 3, ",", after=True)
        assert result and len(result["signatures"]) == 1, result
        assert result["signatures"][0]["parameters"][0]["label"] == f"value: {expected}", result

    check_type("String")
    definition = query("textDocument/definition", 2, "value.")
    assert definition["uri"] == uri, definition
    assert definition["range"]["start"] == {
        "line": 2,
        "character": source.splitlines()[2].index("value"),
    }
    completion = query("textDocument/completion", 2, "value.", after=True)
    assert any(item["label"] == "toString" for item in completion["items"]), completion
    mutable = query("textDocument/hover", 4, "local.")
    assert mutable["contents"]["value"] == "local: Any", mutable
    peer.notify(
        "textDocument/didChange",
        {
            "textDocument": {"uri": uri, "version": 3},
            "contentChanges": [{"text": source.replace("value is String", "value is Boolean")}],
        },
    )
    source = source.replace("value is String", "value is Boolean")
    check_type("Boolean")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    source = source.replace("value is Boolean", "value is String")
    check_type("String")
    assert not (root / "target").exists()
