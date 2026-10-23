"""Iteration previews reuse core protocol and ordinary pattern bindings."""


def check(peer, root):
    path = root / "src/forRecovery.scoop"
    source = (
        "fun forChoice(value: String, next: Boolean): String = value\n"
        "fun forChoice(value: Boolean, next: Boolean): Boolean = value\n"
        "fun forValue(source: Iterable<String>) { cycle@ for (item in source) { "
        "if (false) continue@cycle; item. } }\n"
        "fun forSignature(source: Iterable<String>) { for (item in source) { forChoice(item, } }\n"
        "fun forTuple(source: Iterable<(Boolean, String)>) { for ((flag, text) in source) { text. } }\n"
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
        hover = query("textDocument/hover", 2, "item.")
        assert hover["contents"]["value"] == f"item: {expected}", hover
        result = query("textDocument/signatureHelp", 3, ",", after=True)
        assert result and len(result["signatures"]) == 1, result
        assert result["signatures"][0]["parameters"][0]["label"] == f"value: {expected}", result

    check_type("String")
    for line, name in [(2, "item"), (4, "text")]:
        definition = query("textDocument/definition", line, name + ".")
        assert definition["uri"] == uri, definition
        assert definition["range"]["start"] == {
            "line": line,
            "character": source.splitlines()[line].index(name),
        }, definition
    hover = query("textDocument/hover", 4, "text.")
    assert hover["contents"]["value"] == "text: String", hover
    completion = query("textDocument/completion", 2, "item.", after=True)
    assert any(item["label"] == "toString" for item in completion["items"]), completion
    source = source.replace("Iterable<String>", "Iterable<Boolean>")
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 3}, "contentChanges": [{"text": source}]},
    )
    check_type("Boolean")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    source = source.replace("Iterable<Boolean>", "Iterable<String>")
    check_type("String")
    assert not (root / "target").exists()
