"""Local function recovery retains declaration scopes and source binders."""


def check(peer, root):
    path = root / "src/localFunctionRecovery.scoop"
    source = (
        "fun localChoice(value: String, next: Boolean): Boolean = next\n"
        "fun localChoice(value: Boolean, next: Boolean): Boolean = next\n"
        "fun localHover() { fun 局部(参数: String): Boolean { return@局部 参数. } }\n"
        "fun localSignature() { fun 局部(参数: String) { localChoice(参数, } }\n"
        "fun localGeneric() { fun <Item> 局部(参数: Item) { val nested = { 参数. } } }\n"
        "fun localSelf() { fun <Item> 局部(参数: Item, flag: Boolean = true) = 局部(参数, }\n"
        "fun localCapture(value: String) { fun first() { fun second() { value. } } }\n"
        "fun localContext() { context(参数: String) fun 局部() { 参数. } }\n"
        "fun localUnknown(value: Boolean) { fun local(value: Missing) { value. } }\n"
        "fun localReturn() { fun local(value: String) { return@localReturn value. } }\n"
        "fun <Outer> localScopes(outer: Outer) { if (true) { fun <Inner> local(inner: Inner) { inner. } } else { outer. } }\n"
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
    for line, marker, expected in [
        (4, "参数.", "参数: Item"),
        (6, "value.", "value: String"),
        (7, "参数.", "参数: String"),
        (10, "inner.", "inner: Inner"),
        (10, "outer.", "outer: Outer"),
    ]:
        hover = query("textDocument/hover", line, marker)
        assert hover["contents"]["value"] == expected, hover
    for line, marker, declaration in [
        (2, "参数.", "参数: String"),
        (4, "参数.", "参数: Item"),
        (6, "value.", "value: String"),
        (10, "inner.", "inner: Inner"),
    ]:
        definition = query("textDocument/definition", line, marker)
        assert definition["uri"] == uri, definition
        assert definition["range"]["start"] == {
            "line": line,
            "character": source.splitlines()[line].index(declaration),
        }, definition
    signature = query("textDocument/signatureHelp", 5, ",", after=True)
    assert signature and len(signature["signatures"]) == 1, signature
    assert signature["signatures"][0]["parameters"][0]["label"] == "参数: Item", signature
    completion = query("textDocument/completion", 2, "参数.", after=True)
    assert any(item["label"] == "toString" for item in completion["items"]), completion
    for line in [8, 9]:
        assert query("textDocument/hover", line, "value.") is None
    disk_source = source
    source = source.replace("参数: String", "参数: Boolean")
    peer.notify(
        "textDocument/didChange",
        {"textDocument": {"uri": uri, "version": 3}, "contentChanges": [{"text": source}]},
    )
    check_type("Boolean")
    peer.notify("textDocument/didClose", {"textDocument": {"uri": uri}})
    source = disk_source
    check_type("String")
    assert not (root / "target").exists()
