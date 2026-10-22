"""Callable values navigate to the selected source instead of their adapters."""


def check(peer, root):
    path = root / "src/callableValues.scoop"
    text = (
        "fun choose(value: Boolean): Boolean = value\n"
        "fun choose(value: String): String = value\n"
        "class Value { fun member(): Boolean = true }\n"
        "@NoGC fun native(value: Int): Int = value\n"
        "fun selected(): (Boolean) -> Boolean = ::choose\n"
        "fun bound(value: Value): () -> Boolean = value::member\n"
        "fun unbound(): (Value) -> Boolean = Value::member\n"
        "fun address(): FunPtr<(Int) -> Int> = ::native\n"
        "fun primitive(value: Boolean): () -> String = value::toString\n"
    )
    path.write_text(text)
    uri = path.as_uri()
    peer.notify(
        "textDocument/didOpen",
        {"textDocument": {"uri": uri, "languageId": "scoop", "version": 1, "text": text}},
    )
    for line, name, definition, expected in [
        (4, "choose", 0, "fun choose(value: Boolean): Boolean"),
        (5, "member", 2, "fun member(): Boolean"),
        (6, "member", 2, "fun member(): Boolean"),
        (7, "native", 3, "fun native(value: Int): Int"),
    ]:
        position = {"line": line, "character": text.splitlines()[line].rindex(name)}
        params = {"textDocument": {"uri": uri}, "position": position}
        hover = peer.request("textDocument/hover", params)
        assert hover["contents"]["value"] == expected, hover
        target = peer.request("textDocument/definition", params)
        assert target["uri"] == uri, target
        assert target["range"]["start"]["line"] == definition, target
    position = {"line": 8, "character": text.splitlines()[8].rindex("toString")}
    params = {"textDocument": {"uri": uri}, "position": position}
    hover = peer.request("textDocument/hover", params)
    assert "toString" in hover["contents"]["value"], hover
    assert peer.request("textDocument/definition", params) is None
    assert not peer.diagnostics[uri]["diagnostics"], peer.diagnostics[uri]
