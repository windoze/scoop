"""Incomplete callable bodies retain real parameters, captures, and labels."""


def check(peer, root):
    path = root / "src/callableLiteralRecovery.scoop"
    source = (
        "fun literalChoice(value: String, next: Boolean): Boolean = next\n"
        "fun literalChoice(value: Boolean, next: Boolean): Boolean = next\n"
        "fun literalHover() { val action: (String) -> Boolean = 局部@ { 参数 -> return@局部 参数. } }\n"
        "fun literalSignature() { val action: (String) -> Boolean = { 参数 -> literalChoice(参数, } }\n"
        "fun anonymousHover() { val action = fun(value: String): Boolean { return value. } }\n"
        "fun literalReceiver() { val action = fun String.(): Boolean { this. } }\n"
        "fun literalCapture(value: String) { val action = fun() { val nested = { value. } } }\n"
        "fun literalRejected(value: String) { val action = { return value. } }\n"
        "fun literalUnknown(value: Boolean) { val action = { value -> value. } }\n"
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
        hover = query("textDocument/hover", 2, "参数.")
        assert hover["contents"]["value"] == f"参数: {expected}", hover
        signature = query("textDocument/signatureHelp", 3, ",", after=True)
        assert signature and len(signature["signatures"]) == 1, signature
        assert signature["signatures"][0]["parameters"][0]["label"] == f"value: {expected}", (
            signature
        )

    check_type("String")
    for line, marker, definition_marker in [
        (2, "参数.", "参数 ->"),
        (4, "value.", "value:"),
        (6, "value.", "value:"),
    ]:
        definition = query("textDocument/definition", line, marker)
        assert definition["uri"] == uri, definition
        assert definition["range"]["start"] == {
            "line": line,
            "character": source.splitlines()[line].index(definition_marker),
        }, definition
    completion = query("textDocument/completion", 2, "参数.", after=True)
    assert any(item["label"] == "toString" for item in completion["items"]), completion
    for line, marker, expected in [
        (4, "value.", "value: String"),
        (5, "this.", "String"),
        (6, "value.", "value: String"),
    ]:
        hover = query("textDocument/hover", line, marker)
        assert hover["contents"]["value"] == expected, hover
    for line in [7, 8]:
        assert query("textDocument/hover", line, "value.") is None
    disk_source = source
    source = source.replace("val action: (String) -> Boolean", "val action: (Boolean) -> Boolean")
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 3}, "contentChanges": [{"text": source}]},
    )
    check_type("Boolean")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    source = disk_source
    check_type("String")
    assert not (root / "target").exists()
