"""Do-while previews preserve body scope and condition initialization rules."""


def check(peer, root):
    path = root / "src/doWhileRecovery.scoop"
    source = (
        "fun doChoice(value: Boolean, next: Boolean): Boolean = value\n"
        "fun doChoice(value: String, next: Boolean): Boolean = next\n"
        "fun doBody(value: Any) { do { value. } while (value is String) }\n"
        'fun doCondition() { do { val current = "ready" } while (current.) }\n'
        'fun doSkipped(flag: Boolean) { cycle@ do { if (flag) continue@cycle; val current = "ready" } while (current.) }\n'
        'fun doSignature() { do { val current = "ready" } while (doChoice(current, }\n'
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
        hover = query("textDocument/hover", 3, "current.")
        assert hover["contents"]["value"] == f"current: {expected}", hover
        result = query("textDocument/signatureHelp", 5, ",", after=True)
        assert result and len(result["signatures"]) == 1, result
        assert result["signatures"][0]["parameters"][0]["label"] == f"value: {expected}", result
        assert query("textDocument/hover", 4, "current.") is None

    check_type("String")
    hover = query("textDocument/hover", 2, "value.")
    assert hover["contents"]["value"] == "value: Any", hover
    definition = query("textDocument/definition", 3, "current.")
    assert definition["uri"] == uri, definition
    assert definition["range"]["start"] == {
        "line": 3,
        "character": source.splitlines()[3].index("current"),
    }, definition
    completion = query("textDocument/completion", 3, "current.", after=True)
    assert any(item["label"] == "toString" for item in completion["items"]), completion
    disk_source = source
    source = source.replace('"ready"', "true")
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 3}, "contentChanges": [{"text": source}]},
    )
    check_type("Boolean")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    source = disk_source
    check_type("String")
    assert not (root / "target").exists()
