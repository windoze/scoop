"""When previews share subject, pattern, guard and ordered condition facts."""


def check(peer, root):
    path = root / "src/whenRecovery.scoop"
    source = (
        "fun whenChoice(value: Boolean, next: Boolean): Boolean = next\n"
        "fun whenChoice(value: String, next: Boolean): Boolean = next\n"
        "fun whenValue(value: Any) { when (value) { is String -> { value. }; else -> { value. } } }\n"
        "fun whenPattern(source: (Boolean, String)) { when (source) { case (flag, 文本) -> { 文本. } } }\n"
        "fun whenGuard(source: (Boolean, String)) { when (source) { case (flag, 文本) if whenChoice(文本, } }\n"
        "fun whenCondition(value: Any) { when { value !is String, whenChoice(value, } }\n"
        "fun whenUnknown(value: String) { when { missing -> {}; else -> { value. } } }\n"
    )
    path.write_text(source)
    uri = path.as_uri()
    peer.notify(
        "textDocument/didOpen",
        {"textDocument": {"uri": uri, "languageId": "scoop", "version": 1, "text": source}},
    )

    def query(method, line, marker, after=False, first=False):
        text = source.splitlines()[line]
        offset = text.index(marker) if first else text.rindex(marker)
        if after:
            offset += len(marker)
        character = len(text[:offset].encode("utf-16-le")) // 2
        return peer.request(
            method,
            {"textDocument": {"uri": uri}, "position": {"line": line, "character": character}},
        )

    def check_type(expected):
        hover = query("textDocument/hover", 3, "文本.")
        assert hover["contents"]["value"] == f"文本: {expected}", hover
        result = query("textDocument/signatureHelp", 4, ",", after=True)
        assert result and len(result["signatures"]) == 1, result
        assert result["signatures"][0]["parameters"][0]["label"] == f"value: {expected}", result

    check_type("String")
    for first, expected in [(True, "String"), (False, "Any")]:
        hover = query("textDocument/hover", 2, "value.", first=first)
        assert hover["contents"]["value"] == f"value: {expected}", hover
    definition = query("textDocument/definition", 3, "文本.")
    assert definition["uri"] == uri, definition
    assert definition["range"]["start"] == {
        "line": 3,
        "character": source.splitlines()[3].index("文本"),
    }, definition
    completion = query("textDocument/completion", 3, "文本.", after=True)
    assert any(item["label"] == "toString" for item in completion["items"]), completion
    result = query("textDocument/signatureHelp", 5, ",", after=True)
    assert result and len(result["signatures"]) == 1, result
    assert result["signatures"][0]["parameters"][0]["label"] == "value: String", result
    assert query("textDocument/hover", 6, "value.") is None
    disk_source = source
    source = source.replace("(Boolean, String)", "(Boolean, Boolean)")
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 3}, "contentChanges": [{"text": source}]},
    )
    check_type("Boolean")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    source = disk_source
    check_type("String")
    assert not (root / "target").exists()
